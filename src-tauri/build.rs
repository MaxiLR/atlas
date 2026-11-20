fn main() {
    // Only the commands listed here get permissions, so capabilities can
    // grant exactly the ones the interface uses.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["core_request"])),
    )
    .expect("the Tauri build script failed");
}
