//! The desktop shell: one window, one command into the core, one event out.

use std::sync::Arc;

use atlas_core::ports::EventSink;
use atlas_core::protocol::{CoreError, CoreEvent, CoreRequest, CoreResponse};
use atlas_core::provider::{FakeProvider, FakeScript};
use atlas_core::{Core, CoreDeps};
use tauri::{AppHandle, Emitter, Manager, State};

/// The event that carries every [`CoreEvent`] to the interface.
pub const CORE_EVENT: &str = "core-event";

/// The database file inside the app's data folder.
const DATABASE_FILE: &str = "atlas.sqlite3";

/// The Tauri context compiled from `tauri.conf.json`.
pub fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

struct TauriEvents(AppHandle);

impl EventSink for TauriEvents {
    fn emit(&self, event: CoreEvent) {
        if let Err(error) = self.0.emit(CORE_EVENT, event) {
            eprintln!("could not deliver a core event: {error}");
        }
    }
}

/// The one command: passes a typed request to the core.
#[tauri::command]
async fn core_request(
    core: State<'_, Core>,
    request: CoreRequest,
) -> Result<CoreResponse, CoreError> {
    core.handle(request).await
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let core = Core::new(CoreDeps {
                database_path: data_dir.join(DATABASE_FILE),
                // The fake provider stands in until the ChatGPT plan provider lands.
                provider: Arc::new(FakeProvider::new(FakeScript::sample())),
                events: Arc::new(TauriEvents(app.handle().clone())),
            })?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_request])
        .run(context())
        .expect("Atlas failed to start");
}
