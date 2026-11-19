//! The desktop shell: one window, one command into the core, one event out.

/// The Tauri context compiled from `tauri.conf.json`.
pub fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

pub fn run() {
    tauri::Builder::default()
        .run(context())
        .expect("Atlas failed to start");
}
