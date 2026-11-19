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
