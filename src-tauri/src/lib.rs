//! The desktop shell: one window that shows only the app, one command into
//! the core, one event out, and the core's ports on the Mac (the secrets
//! file, the system browser, the app's data folder, and the PDF printer with
//! its hidden window).

mod navigation;
mod pdf;
mod ports;
pub mod secrets;
#[cfg(test)]
mod tests;

use std::ffi::OsStr;
use std::sync::Arc;

use atlas_core::ports::{Clock, Endpoints, SystemClock};
use atlas_core::protocol::{CoreError, CoreRequest, CoreResponse};
use atlas_core::provider::{ProviderChoice, for_session};
use atlas_core::{Core, CoreDeps};
use tauri::{Emitter, Manager, State, Url};

use pdf::WebviewPdfExporter;
use ports::{ForwardedEvents, SystemBrowser};
use secrets::{SecretsFile, secrets_path};

pub use pdf::{EXPORT_CAPABILITY, EXPORT_WINDOW};

/// The event that carries every core event to the interface.
pub const CORE_EVENT: &str = "core-event";

/// The database file inside the app's data folder.
const DATABASE_FILE: &str = "atlas.sqlite3";

/// Where the bundled interface is served on the Mac.
const BUNDLED_APP: &str = "tauri://localhost";

/// Set to `1`, the core runs the fake provider instead of the plan, for local
/// development without a login.
pub const FAKE_PROVIDER: &str = "ATLAS_FAKE_PROVIDER";

/// The provider for the value of [`FAKE_PROVIDER`]: the fake for `1`, the
/// plan otherwise.
fn provider_choice(value: Option<&OsStr>) -> ProviderChoice {
    match value {
        Some(value) if value == "1" => ProviderChoice::Fake,
        _ => ProviderChoice::Plan,
    }
}

/// The Tauri context compiled from `tauri.conf.json`.
pub fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
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
    let context = context();
    // The development server serves the app only in a development build.
    let dev_url = tauri::is_dev()
        .then(|| context.config().build.dev_url.clone())
        .flatten();
    let app_url = dev_url
        .clone()
        .unwrap_or_else(|| Url::parse(BUNDLED_APP).expect("a valid app URL"));
    tauri::Builder::default()
        .plugin(navigation::plugin(dev_url))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            app.add_capability(EXPORT_CAPABILITY)?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let endpoints = Endpoints::production();
            let clock: Arc<dyn Clock> = Arc::new(SystemClock);
            let choice = provider_choice(std::env::var_os(FAKE_PROVIDER).as_deref());
            let interface = app.handle().clone();
            let core = Core::new(CoreDeps {
                database_path: data_dir.join(DATABASE_FILE),
                provider: for_session(choice, &endpoints, clock.clone()),
                events: Arc::new(ForwardedEvents::start(move |event| {
                    if let Err(error) = interface.emit(CORE_EVENT, event) {
                        eprintln!("could not deliver a core event: {error}");
                    }
                })),
                clock,
                secrets: Arc::new(SecretsFile::new(secrets_path(&data_dir))),
                browser: Arc::new(SystemBrowser(app.handle().clone())),
                pdf: Arc::new(WebviewPdfExporter::new(app.handle().clone(), app_url)),
                endpoints,
            })?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_request])
        .run(context)
        .expect("Atlas failed to start");
}
