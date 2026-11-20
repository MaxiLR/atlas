//! The catalog the core embeds: its prompt text and the validator compiled
//! from its generated JSON schema, held against fixture specs.
//!
//! `fixtures/catalog/valid/` holds one valid spec per catalog entry, named
//! after the entry; `fixtures/catalog/invalid/` holds specs each broken in one
//! way.

use std::fs;
use std::path::PathBuf;

use atlas_core::harness::catalog;
use serde_json::Value;

/// The catalog's entries, as the ticket names them.
const ENTRIES: [&str; 10] = [
    "concept-map",
    "layered-figure",
    "timeline",
    "step-through",
    "comparison",
    "callout",
    "mermaid-diagram",
    "figure",
    "check-question",
    "citation",
];

fn fixture(kind: &str, name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/catalog")
        .join(kind)
        .join(format!("{name}.json"));
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

fn reasons_for(name: &str) -> String {
    match catalog::validate(&fixture("invalid", name)) {
        Ok(()) => panic!("the invalid fixture {name} passed"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn the_catalog_names_every_entry() {
    let mut names = catalog::component_names();
    names.sort_unstable();
    let mut expected = ENTRIES.to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);
}

#[test]
fn one_valid_spec_per_entry_passes() {
    for name in ENTRIES {
        let spec = fixture("valid", name);
        assert_eq!(
            spec["elements"][spec["root"].as_str().unwrap()]["type"],
            name,
            "the fixture {name} is a spec of the entry it is named after"
        );
        if let Err(error) = catalog::validate(&spec) {
            panic!("the valid fixture {name} failed: {error}");
        }
    }
}

#[test]
fn an_unknown_component_fails_naming_it() {
    let reasons = reasons_for("unknown-component");
    assert!(reasons.contains("bar-chart"), "{reasons}");
}

#[test]
fn a_missing_required_prop_fails_naming_it() {
    let reasons = reasons_for("missing-prop");
    assert!(reasons.contains("\"body\""), "{reasons}");
    assert!(reasons.contains("required"), "{reasons}");
}

#[test]
fn a_prop_of_the_wrong_type_fails_naming_where_and_what() {
    let reasons = reasons_for("wrong-type");
    assert!(reasons.contains("/props/title"), "{reasons}");
    assert!(reasons.contains("string"), "{reasons}");
}

#[test]
fn a_spec_whose_root_names_no_element_fails() {
    let mut spec = fixture("valid", "callout");
    spec["root"] = Value::from("missing");
    let error = catalog::validate(&spec).expect_err("a dangling root passed");
    assert!(error.to_string().contains("missing"), "{error}");
}

#[test]
fn the_prompt_describes_every_entry_without_front_matter() {
    let prompt = catalog::prompt();
    assert!(!prompt.starts_with("---"), "{prompt}");
    assert!(!prompt.contains("version:"), "{prompt}");
    for name in ENTRIES {
        assert!(prompt.contains(name), "the prompt does not name {name}");
    }
    assert!(catalog::version() >= 1);
}

#[test]
fn a_citation_to_anything_but_a_web_page_fails() {
    let mut spec = fixture("valid", "citation");
    spec["elements"]["source"]["props"]["url"] = Value::from("javascript:alert(1)");
    let error = catalog::validate(&spec).expect_err("a javascript: link passed");
    assert!(error.to_string().contains("/props/url"), "{error}");
}

#[test]
fn state_actions_and_visibility_are_refused() {
    let mut spec = fixture("valid", "callout");
    spec["elements"]["note"]["visible"] = Value::from(true);
    assert!(catalog::validate(&spec).is_err());
    let mut spec = fixture("valid", "callout");
    spec["elements"]["note"]["props"]["title"] = serde_json::json!({ "$state": "/title" });
    assert!(catalog::validate(&spec).is_err());
}

fn check_question_with(key: u64, misconceptions: [Option<&str>; 4]) -> Value {
    let mut spec = fixture("valid", "check-question");
    let props = &mut spec["elements"]["question"]["props"];
    props["key"] = Value::from(key);
    for (index, misconception) in misconceptions.into_iter().enumerate() {
        props["options"][index]["misconception"] = Value::from(misconception);
    }
    spec
}

#[test]
fn a_check_question_whose_wrong_option_names_no_misconception_fails_naming_it() {
    let spec = check_question_with(
        0,
        [
            None,
            Some("Thinks warmth softens"),
            None,
            Some("Confuses boundaries"),
        ],
    );
    let error =
        catalog::validate(&spec).expect_err("a wrong option without a misconception passed");
    assert!(
        error.to_string().contains("/props/options/2/misconception"),
        "{error}"
    );
}

#[test]
fn a_check_question_whose_right_option_names_a_misconception_fails_naming_it() {
    let spec = check_question_with(
        0,
        [
            Some("Thinks age matters"),
            Some("Thinks warmth softens"),
            Some("Confuses kinds"),
            Some("Confuses boundaries"),
        ],
    );
    let error = catalog::validate(&spec).expect_err("a right option with a misconception passed");
    assert!(
        error.to_string().contains("/props/options/0/misconception"),
        "{error}"
    );
}

#[test]
fn a_check_question_with_misconceptions_only_on_the_right_option_fails() {
    let spec = check_question_with(0, [Some("Thinks age matters"), None, None, None]);
    let error =
        catalog::validate(&spec).expect_err("misconceptions on the wrong options were missing");
    let reasons = error.to_string();
    for index in 0..4 {
        assert!(
            reasons.contains(&format!("/props/options/{index}/misconception")),
            "{reasons}"
        );
    }
}

#[test]
fn a_check_question_keyed_to_another_option_passes_with_its_null_there() {
    let spec = check_question_with(
        3,
        [
            Some("Thinks age matters"),
            Some("Thinks warmth softens"),
            Some("Confuses kinds"),
            None,
        ],
    );
    if let Err(error) = catalog::validate(&spec) {
        panic!("a question keyed to option 3 failed: {error}");
    }
}
