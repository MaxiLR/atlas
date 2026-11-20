//! The shell's compiled configuration.

#[test]
fn the_bundle_is_identified_as_atlas() {
    let context = atlas_app::context();
    let config = context.config();
    assert_eq!(config.identifier, "com.maxilr.atlas");
    assert_eq!(config.product_name.as_deref(), Some("Atlas"));
}

#[test]
fn the_window_is_at_least_720_pixels_wide() {
    let context = atlas_app::context();
    let windows = &context.config().app.windows;
    assert_eq!(windows.len(), 1, "one window");
    assert_eq!(windows[0].min_width, Some(720.0));
    assert!(windows[0].width >= 720.0);
}

#[test]
fn the_content_security_policy_allows_no_remote_script_or_style() {
    let context = atlas_app::context();
    let csp = serde_json::to_value(&context.config().app.security.csp).unwrap();
    let local = ["'self'", "'unsafe-inline'"];
    for directive in ["default-src", "script-src", "style-src"] {
        let sources = csp[directive]
            .as_str()
            .unwrap_or_else(|| panic!("{directive} is set"));
        for source in sources.split_whitespace() {
            assert!(local.contains(&source), "{directive} allows {source}");
        }
    }
    assert!(
        !csp["script-src"]
            .as_str()
            .unwrap()
            .contains("unsafe-inline")
    );
}
