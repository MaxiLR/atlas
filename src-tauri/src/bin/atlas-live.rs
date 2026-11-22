//! `atlas-live`: runs each selected harness task once against the real
//! ChatGPT plan, on a throwaway course, and prints the report. It uses the
//! plan login the app saved in its secrets file and never signs in itself.
//!
//! ```sh
//! cargo run -p atlas-app --bin atlas-live -- --all
//! cargo run -p atlas-app --bin atlas-live -- --task write-section --capture /tmp/streams
//! ```

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use atlas_app::secrets::{SecretsFile, live_secrets_path};
use atlas_core::CoreDeps;
use atlas_core::live::{self, Args, ArgsError, Capture, LiveCore, LiveTask};
use atlas_core::ports::{
    Browser, BrowserError, Clock, Endpoints, EventSink, PdfExporter, PdfOutcome, SystemClock,
};
use atlas_core::protocol::CoreEvent;
use atlas_core::provider::chatgpt::ChatGptProvider;
use tauri::Url;
use uuid::Uuid;

/// How long the warning stands before the first request.
const GRACE: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> ExitCode {
    let args = match live::parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(help @ ArgsError::Help) => {
            println!("{help}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    // The throwaway course's store, removed after the run.
    let dir = std::env::temp_dir().join(format!("atlas-live-{}", Uuid::new_v4()));
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("the throwaway store could not be created: {error}");
        return ExitCode::FAILURE;
    }
    let code = run(args, &dir).await;
    let _ = std::fs::remove_dir_all(&dir);
    code
}

async fn run(args: Args, dir: &Path) -> ExitCode {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("HOME is not set, so the app's secrets file cannot be found");
        return ExitCode::FAILURE;
    };
    let endpoints = Endpoints::production();
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let capture = args
        .capture
        .as_deref()
        .map(|dir| Arc::new(Capture::new(dir)));
    let provider = ChatGptProvider::awaiting_login(&endpoints, clock.clone());
    let provider = match &capture {
        Some(capture) => provider.with_capture(capture.clone()),
        None => provider,
    };
    let core = LiveCore::new(CoreDeps {
        database_path: dir.join("atlas.sqlite3"),
        provider: Arc::new(provider),
        events: Arc::new(Quiet),
        clock,
        secrets: Arc::new(SecretsFile::new(live_secrets_path(Path::new(&home)))),
        browser: Arc::new(NoBrowser),
        pdf: Arc::new(NoPdf),
        endpoints,
    });
    let core = match core {
        Ok(core) => core.with_model(args.model),
        Err(error) => {
            eprintln!("the core could not start: {error}");
            return ExitCode::FAILURE;
        }
    };
    let core = match capture {
        Some(capture) => core.with_capture(capture),
        None => core,
    };
    if let Err(error) = core.require_login().await {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }

    let requests = args.tasks.len() + usize::from(args.tasks.contains(&LiveTask::WriteSection));
    println!(
        "This run spends plan usage: {requests} requests to the ChatGPT plan, one per task \
         (two for write-section), plus any repairs. Press Ctrl-C within {} seconds to stop.",
        GRACE.as_secs()
    );
    tokio::time::sleep(GRACE).await;

    match live::run(&args.tasks, &core, &mut std::io::stdout()).await {
        Ok(report) => {
            println!("\n{}", report.table());
            if report.failed() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// The interface's events have no window to go to.
struct Quiet;

impl EventSink for Quiet {
    fn emit(&self, _event: CoreEvent) {}
}

/// The command never signs in, so it opens nothing.
struct NoBrowser;

impl Browser for NoBrowser {
    fn open(&self, _url: &Url) -> Result<(), BrowserError> {
        Err(BrowserError(
            "the live command opens no browser; sign in with the app".to_owned(),
        ))
    }
}

/// The command exports nothing.
struct NoPdf;

#[async_trait]
impl PdfExporter for NoPdf {
    async fn export(&self, _course_id: Uuid, _suggested_name: &str) -> PdfOutcome {
        PdfOutcome::Failed("the live command exports nothing".to_owned())
    }
}
