//! The PDF exporter on the Mac: the save dialog picks the file, a hidden
//! window loads the course's export route, and the webview's print operation
//! writes the pages straight to the file.

use std::path::{Path, PathBuf};
use std::time::Duration;

use atlas_core::ports::PdfOutcome;
use tauri::Url;
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

/// The label of the hidden window that loads the export route.
pub const EXPORT_WINDOW: &str = "pdf-export";

/// What the hidden export view may call: the one command, to read the
/// course, and the core's event, which every page of the app listens to.
pub const EXPORT_CAPABILITY: &str = "";

/// What the export route signalled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewState {
    Ready,
    Failed,
}

/// How a print operation lays the document out along one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pagination {
    /// Breaks the document into as many pages as it needs.
    Automatic,
    /// Scales the document to fit the page.
    Fit,
}

/// A paper size, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
    pub name: &'static str,
    pub width: f64,
    pub height: f64,
}

/// Page margins, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Everything the print operation is given.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintSettings {
    /// The file the pages are written to.
    pub destination: PathBuf,
    pub paper: Paper,
    pub margins: Margins,
    pub horizontal_pagination: Pagination,
    pub vertical_pagination: Pagination,
    pub shows_print_panel: bool,
    pub shows_progress_panel: bool,
}

/// How long the export waits for each step that depends on the webview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeouts {
    /// From loading the export route to its ready signal.
    pub ready: Duration,
    /// From starting the print operation to its end.
    pub print: Duration,
}

/// The native steps of an export.
pub trait ExportSteps: Sync {
    /// Asks for the file with the save dialog; `None` when it was dismissed.
    fn choose_destination(&self, file_name: &str) -> impl Future<Output = Option<PathBuf>> + Send;
    /// Loads `url` in a view that is never shown; the receiver yields what it
    /// signals.
    fn open_view(&self, url: &Url) -> Result<UnboundedReceiver<ViewState>, String>;
    /// Prints the open view with `settings`.
    fn print(&self, settings: &PrintSettings) -> impl Future<Output = Result<(), String>> + Send;
    /// Closes the view, whether or not it is ready.
    fn close_view(&self);
}

/// The file name the save dialog suggests for the core's `suggested` name.
pub fn suggested_file_name(_suggested: &str) -> String {
    unimplemented!()
}

/// The export route of `course_id` in the app served at `app_url`.
pub fn export_url(_app_url: &Url, _course_id: Uuid) -> Url {
    unimplemented!()
}

/// The print operation's settings for writing to `destination`.
pub fn print_settings(_destination: &Path) -> PrintSettings {
    unimplemented!()
}

/// The state the export view reported in its document title, if any.
pub fn view_state(_title: &str) -> Option<ViewState> {
    unimplemented!()
}

/// Exports the course through `steps`.
pub async fn export_with(
    _steps: &impl ExportSteps,
    _app_url: &Url,
    _course_id: Uuid,
    _suggested_name: &str,
    _timeouts: Timeouts,
) -> PdfOutcome {
    unimplemented!()
}
