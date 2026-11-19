use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use atlas_core::ports::PdfOutcome;
use tauri::Url;
use tauri::utils::acl::capability::CapabilityFile;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use crate::navigation::is_app_url;
use crate::pdf::{
    EXPORT_CAPABILITY, EXPORT_WINDOW, ExportSteps, Pagination, PrintSettings, Timeouts, ViewState,
    export_url, export_with, print_settings, suggested_file_name, view_state,
};

const COURSE: &str = "0192f0c4-3b7e-7a59-9a4e-2d6f1c8b5e01";

fn course() -> Uuid {
    Uuid::parse_str(COURSE).unwrap()
}

fn url(text: &str) -> Url {
    Url::parse(text).expect("a valid URL")
}

// The suggested file name.

#[test]
fn slashes_and_colons_become_spaces() {
    assert_eq!(
        suggested_file_name("TCP/IP networking.pdf"),
        "TCP IP networking.pdf"
    );
    assert_eq!(
        suggested_file_name("Rust: ownership and borrowing.pdf"),
        "Rust ownership and borrowing.pdf"
    );
    assert_eq!(
        suggested_file_name(r"Windows\paths | pipes.pdf"),
        "Windows paths pipes.pdf"
    );
}

#[test]
fn a_name_never_climbs_out_of_the_folder_or_hides_itself() {
    assert_eq!(
        suggested_file_name("../../etc/passwd.pdf"),
        "etc passwd.pdf"
    );
    assert_eq!(suggested_file_name(".hidden.pdf"), "hidden.pdf");
}

#[test]
fn emoji_and_control_characters_are_dropped() {
    assert_eq!(
        suggested_file_name("🚀 Rocket propulsion 🔥.pdf"),
        "Rocket propulsion.pdf"
    );
    assert_eq!(
        suggested_file_name("Line\nbreak\tand\u{7}bell.pdf"),
        "Line break and bell.pdf"
    );
}

#[test]
fn letters_of_any_script_and_ordinary_punctuation_are_kept() {
    for name in [
        "Théorie des catégories.pdf",
        "Node.js, Deno & Bun (2026).pdf",
        "量子力学.pdf",
        "C++ templates - a primer.pdf",
    ] {
        assert_eq!(suggested_file_name(name), name);
    }
}

#[test]
fn the_name_always_ends_in_pdf_once() {
    assert_eq!(suggested_file_name("Linear algebra"), "Linear algebra.pdf");
    assert_eq!(
        suggested_file_name("Linear algebra.PDF"),
        "Linear algebra.pdf"
    );
    assert_eq!(
        suggested_file_name("Linear algebra.pdf"),
        "Linear algebra.pdf"
    );
}

#[test]
fn a_very_long_topic_is_cut_to_a_hundred_characters_on_a_word() {
    let topic = "Distributed systems ".repeat(20);
    let name = suggested_file_name(&format!("{topic}.pdf"));
    let stem = name.strip_suffix(".pdf").expect("a PDF name");
    assert!(stem.chars().count() <= 100, "{name} is too long");
    assert!(
        stem.starts_with("Distributed systems Distributed"),
        "{name}"
    );
    assert!(!stem.ends_with(' '), "{name:?} ends in a space");

    let accented = "é".repeat(300);
    assert_eq!(
        suggested_file_name(&format!("{accented}.pdf")),
        format!("{}.pdf", "é".repeat(100))
    );
}

#[test]
fn an_empty_topic_gets_a_name_of_its_own() {
    for empty in ["", ".pdf", "   .pdf", "🚀🔥.pdf", "/:/.pdf", "....pdf"] {
        assert_eq!(
            suggested_file_name(empty),
            "Atlas course.pdf",
            "{empty:?} has a name"
        );
    }
}

// The export route.

#[test]
fn the_export_route_is_the_app_page_with_the_course_in_its_hash() {
    assert_eq!(
        export_url(&url("tauri://localhost"), course()).as_str(),
        format!("tauri://localhost/#/export/{COURSE}")
    );
    assert_eq!(
        export_url(&url("http://localhost:1420"), course()).as_str(),
        format!("http://localhost:1420/#/export/{COURSE}")
    );
}

#[test]
fn the_export_route_drops_the_page_query_and_hash_it_starts_from() {
    assert_eq!(
        export_url(&url("http://localhost:1420/reader?x=1#top"), course()).as_str(),
        format!("http://localhost:1420/#/export/{COURSE}")
    );
}

#[test]
fn the_navigation_rule_lets_the_hidden_view_load_the_export_route() {
    let dev = url("http://localhost:1420");
    assert!(is_app_url(
        &export_url(&url("tauri://localhost"), course()),
        None
    ));
    assert!(is_app_url(&export_url(&dev, course()), Some(&dev)));
}

// The print settings.

#[test]
fn the_print_operation_writes_paginated_a4_pages_to_the_file_without_panels() {
    let destination = Path::new("/tmp/atlas/Rust ownership.pdf");
    let settings = print_settings(destination);

    assert_eq!(settings.destination, destination);
    assert_eq!(settings.paper.name, "iso-a4");
    assert_eq!(
        (settings.paper.width, settings.paper.height),
        (595.0, 842.0)
    );
    assert_eq!(settings.vertical_pagination, Pagination::Automatic);
    assert_eq!(settings.horizontal_pagination, Pagination::Fit);
    assert!(!settings.shows_print_panel, "no print panel");
    assert!(!settings.shows_progress_panel, "no progress panel");
}

#[test]
fn the_print_margins_are_the_export_route_s_page_margins() {
    // The route asks for `@page { margin: 18mm 16mm; }`: 51.02 and 45.35 pt.
    let margins = print_settings(Path::new("/tmp/x.pdf")).margins;
    for (side, found, expected) in [
        ("top", margins.top, 51.02),
        ("bottom", margins.bottom, 51.02),
        ("left", margins.left, 45.35),
        ("right", margins.right, 45.35),
    ] {
        assert!(
            (found - expected).abs() < 0.01,
            "{side} margin {found} is not {expected}"
        );
    }
}

// The ready signal.

#[test]
fn the_view_reports_its_state_in_its_title() {
    assert_eq!(view_state("atlas-export:ready"), Some(ViewState::Ready));
    assert_eq!(view_state("atlas-export:failed"), Some(ViewState::Failed));
    for other in [
        "Atlas",
        "",
        "atlas-export:",
        "atlas-export:loading",
        "ready",
    ] {
        assert_eq!(view_state(other), None, "{other:?} is no state");
    }
}

// The export capability.

#[test]
fn the_export_capability_is_a_valid_capability_for_the_hidden_window_only() {
    let parsed: CapabilityFile = EXPORT_CAPABILITY.parse().expect("a capability");
    let CapabilityFile::Capability(capability) = parsed else {
        panic!("one capability");
    };
    assert_eq!(capability.windows, [EXPORT_WINDOW]);
    assert!(capability.webviews.is_empty());
    assert!(capability.remote.is_none());
    assert_ne!(EXPORT_WINDOW, "main");
}

// The export, through fake native steps.

/// What the fake view does once it is loaded.
enum View {
    Signals(ViewState),
    /// Never signals, and stays open.
    Silent,
}

/// What the fake print operation does.
enum Print {
    Writes(&'static str),
    Fails(&'static str),
    Hangs,
}

struct FakeSteps {
    destination: Option<PathBuf>,
    view: View,
    print: Print,
    asked: Mutex<Vec<String>>,
    opened: Mutex<Vec<Url>>,
    printed: Mutex<Vec<PrintSettings>>,
    closed: Mutex<usize>,
    /// Keeps a silent view's channel open.
    silent: Mutex<Option<UnboundedSender<ViewState>>>,
}

impl FakeSteps {
    fn new(destination: Option<PathBuf>, view: View, print: Print) -> Self {
        Self {
            destination,
            view,
            print,
            asked: Mutex::default(),
            opened: Mutex::default(),
            printed: Mutex::default(),
            closed: Mutex::default(),
            silent: Mutex::default(),
        }
    }
}

impl ExportSteps for FakeSteps {
    async fn choose_destination(&self, file_name: &str) -> Option<PathBuf> {
        self.asked.lock().unwrap().push(file_name.to_owned());
        self.destination.clone()
    }

    fn open_view(&self, url: &Url) -> Result<UnboundedReceiver<ViewState>, String> {
        self.opened.lock().unwrap().push(url.clone());
        let (sender, receiver) = unbounded_channel();
        match self.view {
            View::Signals(state) => sender.send(state).unwrap(),
            View::Silent => *self.silent.lock().unwrap() = Some(sender),
        }
        Ok(receiver)
    }

    async fn print(&self, settings: &PrintSettings) -> Result<(), String> {
        self.printed.lock().unwrap().push(settings.clone());
        match self.print {
            Print::Writes(text) => {
                std::fs::write(&settings.destination, text).map_err(|error| error.to_string())
            }
            Print::Fails(reason) => {
                // A failed operation may leave part of a file behind.
                std::fs::write(&settings.destination, "%PDF-1.7 trunc").unwrap();
                Err(reason.to_owned())
            }
            Print::Hangs => std::future::pending().await,
        }
    }

    fn close_view(&self) {
        *self.closed.lock().unwrap() += 1;
    }
}

const QUICK: Timeouts = Timeouts {
    ready: Duration::from_millis(50),
    print: Duration::from_millis(50),
};

const PATIENT: Timeouts = Timeouts {
    ready: Duration::from_secs(10),
    print: Duration::from_secs(10),
};

fn app() -> Url {
    url("tauri://localhost")
}

/// The names of the files in `dir`, sorted.
fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn a_dismissed_dialog_cancels_the_export_without_loading_the_view() {
    let steps = FakeSteps::new(None, View::Signals(ViewState::Ready), Print::Writes("x"));

    let outcome = export_with(&steps, &app(), course(), "TCP/IP.pdf", PATIENT).await;

    assert_eq!(outcome, PdfOutcome::Cancelled);
    assert_eq!(*steps.asked.lock().unwrap(), ["TCP IP.pdf"]);
    assert!(steps.opened.lock().unwrap().is_empty(), "no view is loaded");
    assert!(
        steps.printed.lock().unwrap().is_empty(),
        "nothing is printed"
    );
}

#[tokio::test]
async fn a_view_that_never_signals_ready_fails_after_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("Course.pdf");
    let steps = FakeSteps::new(Some(destination.clone()), View::Silent, Print::Writes("x"));

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        export_with(&steps, &app(), course(), "Course.pdf", QUICK),
    )
    .await
    .expect("the export ends on its own timeout");

    let PdfOutcome::Failed(reason) = outcome else {
        panic!("a failure, found {outcome:?}");
    };
    assert!(reason.contains("ready"), "the reason says why: {reason}");
    assert!(
        steps.printed.lock().unwrap().is_empty(),
        "nothing is printed"
    );
    assert_eq!(*steps.closed.lock().unwrap(), 1, "the view is closed");
    assert!(files_in(dir.path()).is_empty(), "no file is written");
}

#[tokio::test]
async fn a_view_that_could_not_read_the_course_fails_without_printing() {
    let dir = tempfile::tempdir().unwrap();
    let steps = FakeSteps::new(
        Some(dir.path().join("Course.pdf")),
        View::Signals(ViewState::Failed),
        Print::Writes("x"),
    );

    let outcome = export_with(&steps, &app(), course(), "Course.pdf", PATIENT).await;

    assert!(matches!(outcome, PdfOutcome::Failed(_)), "{outcome:?}");
    assert!(
        steps.printed.lock().unwrap().is_empty(),
        "nothing is printed"
    );
    assert_eq!(*steps.closed.lock().unwrap(), 1, "the view is closed");
    assert!(files_in(dir.path()).is_empty(), "no file is written");
}

#[tokio::test]
async fn a_ready_view_is_printed_to_the_chosen_file_and_saved() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("Rust ownership.pdf");
    let steps = FakeSteps::new(
        Some(destination.clone()),
        View::Signals(ViewState::Ready),
        Print::Writes("%PDF-1.7 pages"),
    );

    let outcome = export_with(&steps, &app(), course(), "Rust: ownership.pdf", PATIENT).await;

    assert_eq!(outcome, PdfOutcome::Saved(destination.clone()));
    assert_eq!(*steps.asked.lock().unwrap(), ["Rust ownership.pdf"]);
    assert_eq!(
        *steps.opened.lock().unwrap(),
        [url(&format!("tauri://localhost/#/export/{COURSE}"))]
    );
    let printed = steps.printed.lock().unwrap().clone();
    assert_eq!(printed.len(), 1, "one print operation");
    assert!(!printed[0].shows_print_panel && !printed[0].shows_progress_panel);
    assert_eq!(*steps.closed.lock().unwrap(), 1, "the view is closed");
    assert_eq!(files_in(dir.path()), ["Rust ownership.pdf"], "one file");
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "%PDF-1.7 pages"
    );
}

#[tokio::test]
async fn a_confirmed_existing_file_is_replaced_once_the_print_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("Course.pdf");
    std::fs::write(&destination, "an older export").unwrap();
    let steps = FakeSteps::new(
        Some(destination.clone()),
        View::Signals(ViewState::Ready),
        Print::Writes("%PDF-1.7 new"),
    );

    let outcome = export_with(&steps, &app(), course(), "Course.pdf", PATIENT).await;

    assert_eq!(outcome, PdfOutcome::Saved(destination.clone()));
    assert_eq!(files_in(dir.path()), ["Course.pdf"], "one file");
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "%PDF-1.7 new"
    );
}

#[tokio::test]
async fn a_failed_print_reports_its_reason_and_leaves_an_existing_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("Course.pdf");
    std::fs::write(&destination, "an older export").unwrap();
    let steps = FakeSteps::new(
        Some(destination.clone()),
        View::Signals(ViewState::Ready),
        Print::Fails("the printer said no"),
    );

    let outcome = export_with(&steps, &app(), course(), "Course.pdf", PATIENT).await;

    assert_eq!(
        outcome,
        PdfOutcome::Failed("the printer said no".to_owned())
    );
    assert_eq!(*steps.closed.lock().unwrap(), 1, "the view is closed");
    assert_eq!(files_in(dir.path()), ["Course.pdf"], "no partial file");
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        "an older export"
    );
}

#[tokio::test]
async fn a_print_that_never_ends_fails_after_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let steps = FakeSteps::new(
        Some(dir.path().join("Course.pdf")),
        View::Signals(ViewState::Ready),
        Print::Hangs,
    );

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        export_with(&steps, &app(), course(), "Course.pdf", QUICK),
    )
    .await
    .expect("the export ends on its own timeout");

    assert!(matches!(outcome, PdfOutcome::Failed(_)), "{outcome:?}");
    assert_eq!(*steps.closed.lock().unwrap(), 1, "the view is closed");
    assert!(files_in(dir.path()).is_empty(), "no file is written");
}
