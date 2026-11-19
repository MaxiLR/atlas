//! The webview's policy, read from the configuration and capability files:
//! everything the model writes is untrusted, so the webview can load nothing
//! remote and call nothing but the core.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The only permissions the webview may hold: listening to the core's event,
/// dragging the window by its bar, and the one command into the core.
const ALLOWED_PERMISSIONS: [&str; 4] = [
    "allow-core-request",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "core:window:allow-start-dragging",
];

/// Tauri's own IPC endpoints: the custom protocol and its fallback.
const IPC_ENDPOINTS: [&str; 2] = ["ipc:", "http://ipc.localhost"];

fn shell_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

fn config() -> Value {
    read_json(&shell_dir().join("tauri.conf.json"))
}

/// The content security policy as directive → sources, whether the
/// configuration writes it as one string or as an object.
fn csp() -> BTreeMap<String, Vec<String>> {
    let config = config();
    let csp = &config["app"]["security"]["csp"];
    let directives: Vec<(String, String)> = match csp {
        Value::String(policy) => policy
            .split(';')
            .filter_map(|directive| {
                let directive = directive.trim();
                let (name, sources) = directive.split_once(' ').unwrap_or((directive, ""));
                (!name.is_empty()).then(|| (name.to_owned(), sources.to_owned()))
            })
            .collect(),
        Value::Object(map) => map
            .iter()
            .map(|(name, sources)| {
                let sources = match sources {
                    Value::String(sources) => sources.clone(),
                    Value::Array(sources) => sources
                        .iter()
                        .map(|source| source.as_str().expect("a source is a string"))
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => panic!("{name} has sources {other}"),
                };
                (name.clone(), sources)
            })
            .collect(),
        other => panic!("the content security policy is set, found {other}"),
    };
    directives
        .into_iter()
        .map(|(name, sources)| {
            let sources = sources.split_whitespace().map(str::to_owned).collect();
            (name, sources)
        })
        .collect()
}

fn sources<'a>(csp: &'a BTreeMap<String, Vec<String>>, directive: &str) -> BTreeSet<&'a str> {
    csp.get(directive)
        .unwrap_or_else(|| panic!("{directive} is set"))
        .iter()
        .map(String::as_str)
        .collect()
}

/// Every capability file in `capabilities/`, by file name.
fn capabilities() -> Vec<(String, Value)> {
    let dir = shell_dir().join("capabilities");
    let mut files: Vec<(String, Value)> = std::fs::read_dir(&dir)
        .expect("the capabilities folder exists")
        .map(|entry| entry.expect("a readable entry").path())
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(
                name.ends_with(".json"),
                "capability {name} is JSON, so this test reads it"
            );
            (name, read_json(&path))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn permission_identifier(permission: &Value) -> String {
    match permission {
        Value::String(identifier) => identifier.clone(),
        Value::Object(map) => map["identifier"]
            .as_str()
            .expect("a scoped permission names its identifier")
            .to_owned(),
        other => panic!("a permission is a string or an object, found {other}"),
    }
}

/// A source is local when it is a keyword (`'self'`, `'none'`, …) or a scheme
/// that never leaves the machine; hosts, wildcards and network schemes such
/// as `https:` reach remote origins.
fn is_remote(source: &str) -> bool {
    let keyword = source.starts_with('\'') && source.ends_with('\'');
    let local_scheme = ["data:", "blob:"].contains(&source);
    !(keyword || local_scheme)
}

#[test]
fn scripts_come_only_from_the_app() {
    let csp = csp();
    for directive in ["default-src", "script-src"] {
        assert_eq!(
            sources(&csp, directive),
            BTreeSet::from(["'self'"]),
            "{directive} allows the app's own files and nothing else"
        );
    }
    for directive in ["script-src-elem", "script-src-attr", "worker-src"] {
        if let Some(found) = csp.get(directive) {
            assert_eq!(found, &["'self'"], "{directive} allows only the app");
        }
    }
}

#[test]
fn no_source_allows_evaluated_or_inline_script() {
    let csp = csp();
    for (directive, found) in &csp {
        for source in found {
            assert!(
                !source.contains("unsafe-eval") && source != "'unsafe-hashes'",
                "{directive} allows evaluated script: {source}"
            );
            if directive.starts_with("script-src") || directive == "default-src" {
                assert_ne!(
                    source, "'unsafe-inline'",
                    "{directive} allows inline script"
                );
            }
        }
    }
}

#[test]
fn styles_come_from_the_app_with_inline_styles_for_katex_and_mermaid() {
    let csp = csp();
    assert_eq!(
        sources(&csp, "style-src"),
        BTreeSet::from(["'self'", "'unsafe-inline'"])
    );
    for directive in ["style-src-elem", "style-src-attr"] {
        if let Some(found) = csp.get(directive) {
            for source in found {
                assert!(
                    ["'self'", "'unsafe-inline'"].contains(&source.as_str()),
                    "{directive} allows {source}"
                );
            }
        }
    }
}

#[test]
fn no_directive_names_a_remote_origin_except_the_ipc_endpoints() {
    let csp = csp();
    for (directive, found) in &csp {
        for source in found {
            if directive == "connect-src" && IPC_ENDPOINTS.contains(&source.as_str()) {
                continue;
            }
            assert!(
                !is_remote(source),
                "{directive} names a remote origin: {source}"
            );
        }
    }
}

#[test]
fn connections_go_only_to_tauri_ipc() {
    let csp = csp();
    assert_eq!(sources(&csp, "connect-src"), BTreeSet::from(IPC_ENDPOINTS));
}

#[test]
fn images_come_from_the_app_and_data_urls_and_fonts_from_the_app() {
    let csp = csp();
    assert_eq!(
        sources(&csp, "img-src"),
        BTreeSet::from(["'self'", "data:"])
    );
    assert_eq!(sources(&csp, "font-src"), BTreeSet::from(["'self'"]));
}

#[test]
fn no_frames_objects_or_base_uri() {
    let csp = csp();
    for directive in ["frame-src", "object-src", "base-uri"] {
        assert_eq!(
            sources(&csp, directive),
            BTreeSet::from(["'none'"]),
            "{directive} allows nothing"
        );
    }
}

#[test]
fn the_development_policy_is_not_loosened() {
    let config = config();
    let security = &config["app"]["security"];
    assert!(
        security.get("devCsp").is_none_or(Value::is_null),
        "the policy holds in development too"
    );
    assert!(
        security
            .get("dangerousDisableAssetCspModification")
            .is_none_or(|value| value == &json!(false)),
        "Tauri adds its hashes and nonces to the policy"
    );
}

#[test]
fn one_capability_grants_the_main_window_exactly_the_allow_list() {
    let capabilities = capabilities();
    assert_eq!(capabilities.len(), 1, "one capability file");
    let (name, capability) = &capabilities[0];
    assert_eq!(capability["windows"], json!(["main"]), "{name} is for main");
    assert!(
        capability.get("webviews").is_none(),
        "{name} names no extra webview"
    );
    let granted: BTreeSet<String> = capability["permissions"]
        .as_array()
        .expect("permissions is a list")
        .iter()
        .map(permission_identifier)
        .collect();
    let allowed: BTreeSet<String> = ALLOWED_PERMISSIONS.map(str::to_owned).into();
    assert_eq!(granted, allowed);
}

#[test]
fn no_capability_reaches_a_remote_domain() {
    for (name, capability) in capabilities() {
        assert!(
            capability.get("remote").is_none(),
            "{name} opens the core to remote pages"
        );
    }
}

#[test]
fn the_configuration_adds_no_inline_capability() {
    let config = config();
    let inline = &config["app"]["security"]["capabilities"];
    assert!(
        inline.is_null() || inline == &json!([]),
        "capabilities live in capabilities/ only, found {inline}"
    );
}

#[test]
fn the_app_manifest_makes_the_single_command_explicit() {
    let generated: Vec<String> = std::fs::read_dir(shell_dir().join("permissions/autogenerated"))
        .expect("tauri-build generates the command permissions")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(generated, ["core_request.toml"]);
}

#[test]
fn the_global_tauri_object_is_off() {
    assert_eq!(config()["app"]["withGlobalTauri"], json!(false));
}

#[test]
fn the_app_is_identified_as_atlas() {
    assert_eq!(config()["identifier"], json!("com.maxilr.atlas"));
}

#[test]
fn the_main_window_is_at_least_720_wide_with_an_overlaid_title_bar() {
    let config = config();
    let windows = config["app"]["windows"].as_array().expect("windows");
    assert_eq!(windows.len(), 1, "one window");
    let window = &windows[0];
    assert_eq!(window["label"], json!("main"));
    assert_eq!(window["minWidth"], json!(720));
    assert!(window["width"].as_f64().expect("a width") >= 720.0);
    assert_eq!(window["titleBarStyle"], json!("Overlay"));
    assert_eq!(window["hiddenTitle"], json!(true));
}
