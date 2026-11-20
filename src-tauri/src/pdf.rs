//! The PDF exporter on the Mac. The save dialog picks the file; a window
//! that is never shown loads the course's export route; once the route
//! signals ready, the webview's print operation writes paginated pages to a
//! file of the export's own, with no print or progress panel, and the whole
//! PDF is moved onto the chosen file.
//!
//! Everything that does not need a window (the file name, the route, the
//! print settings and the export's steps with their timeouts) is tested; the
//! dialog, the window and the print operation are checked by hand.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use atlas_core::ports::{PdfExporter, PdfOutcome};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio::sync::oneshot;
use uuid::Uuid;

/// The label of the hidden window that loads the export route.
pub const EXPORT_WINDOW: &str = "pdf-export";

/// What the hidden export window may call: the one command, to read the
/// course, and the core's event, which every page of the app listens to.
/// Added at start-up, beside the main window's capability file.
pub const EXPORT_CAPABILITY: &str = r#"{
  "identifier": "pdf-export",
  "description": "The hidden window that prints a course: it reads the course through the core and listens to the core's event, like every page of the app.",
  "windows": ["pdf-export"],
  "permissions": [
    "allow-core-request",
    "core:event:allow-listen",
    "core:event:allow-unlisten"
  ]
}"#;

/// How long a real export waits for the route to draw the course, and for
/// the print operation to write it.
const TIMEOUTS: Timeouts = Timeouts {
    ready: Duration::from_secs(60),
    print: Duration::from_secs(120),
};

/// The longest file name the dialog suggests, before `.pdf`, in characters.
const MAX_NAME_CHARS: usize = 100;

/// The suggested name when the topic has no safe character.
const UNNAMED: &str = "Atlas course";

/// The prefix of the document title through which the export view reports
/// its state: the route signals it with an event the page cannot send to the
/// shell without a permission, so a script injected into the view copies it
/// into the title, which the shell observes.
const STATE_TITLE: &str = "atlas-export:";

/// Copies each state the export route signals (`atlas:export-state`, see
/// `ui/src/screens/export/ready.ts`) into the document title.
const STATE_SCRIPT: &str = r#"window.addEventListener("atlas:export-state", (event) => {
  document.title = "atlas-export:" + event.detail;
});"#;

/// The size the hidden window lays the route out at before printing: an A4
/// page at 96 pixels per inch.
const VIEW_SIZE: (f64, f64) = (794.0, 1123.0);

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
    /// Cancels the print operation that [`ExportSteps::print`] started and
    /// that the export stopped waiting for. Steps whose print stops when its
    /// future is dropped have nothing more to cancel.
    fn cancel_print(&self) {}
    /// Closes the view, whether or not it is ready.
    fn close_view(&self);
}

/// The file name the save dialog suggests for the core's `suggested` name
/// (the topic and `.pdf`): letters and digits of any script, spaces and
/// ordinary punctuation, at most [`MAX_NAME_CHARS`] characters cut at a
/// word, never starting with a dot, and ending in `.pdf`. Slashes, colons,
/// emoji and control characters become spaces.
pub fn suggested_file_name(suggested: &str) -> String {
    let spaced: String = without_pdf_extension(suggested)
        .chars()
        .map(|c| if is_safe(c) { c } else { ' ' })
        .collect();
    let words = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let stem = trim_name(cut_at_word(trim_name(&words), MAX_NAME_CHARS));
    format!("{}.pdf", if stem.is_empty() { UNNAMED } else { stem })
}

fn without_pdf_extension(name: &str) -> &str {
    let start = name.len().saturating_sub(".pdf".len());
    match name.get(start..) {
        Some(extension) if extension.eq_ignore_ascii_case(".pdf") => &name[..start],
        _ => name,
    }
}

fn is_safe(c: char) -> bool {
    c.is_alphanumeric() || " -_.,()'&+".contains(c)
}

/// Without leading or trailing spaces, dots and hyphens: no hidden file, no
/// name read as an option.
fn trim_name(name: &str) -> &str {
    name.trim_matches([' ', '.', '-'])
}

/// The first `max` characters of `text`, without a last word that the cut
/// splits, unless that word is all there is.
fn cut_at_word(text: &str, max: usize) -> &str {
    let Some((end, next)) = text.char_indices().nth(max) else {
        return text;
    };
    let cut = &text[..end];
    match cut.rfind(' ') {
        Some(space) if next != ' ' => &cut[..space],
        _ => cut,
    }
}

/// The export route of `course_id` in the app served at `app_url`.
pub fn export_url(app_url: &Url, course_id: Uuid) -> Url {
    let mut url = app_url.clone();
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(Some(&format!("/export/{course_id}")));
    url
}

/// ISO A4, in points.
const A4: Paper = Paper {
    name: "iso-a4",
    width: 595.0,
    height: 842.0,
};

/// `length` millimetres in points.
const fn millimetres(length: f64) -> f64 {
    length * 72.0 / 25.4
}

/// The print operation's settings for writing to `destination`: A4 pages
/// with the export route's margins (`@page { margin: 18mm 16mm }`), as many
/// as the course needs, each as wide as the page, with no panel shown.
pub fn print_settings(destination: &Path) -> PrintSettings {
    PrintSettings {
        destination: destination.to_owned(),
        paper: A4,
        margins: Margins {
            top: millimetres(18.0),
            right: millimetres(16.0),
            bottom: millimetres(18.0),
            left: millimetres(16.0),
        },
        horizontal_pagination: Pagination::Fit,
        vertical_pagination: Pagination::Automatic,
        shows_print_panel: false,
        shows_progress_panel: false,
    }
}

/// The state the export view reported in its document title, if any.
pub fn view_state(title: &str) -> Option<ViewState> {
    match title.strip_prefix(STATE_TITLE)? {
        "ready" => Some(ViewState::Ready),
        "failed" => Some(ViewState::Failed),
        _ => None,
    }
}

/// Exports the course through `steps`: asks for the file, loads the export
/// route, waits for it to be ready and prints it. The pages are written to a
/// file of this export's own in the temporary folder and moved onto the
/// chosen file once printed, so a file the dialog confirmed replacing is
/// replaced only by a whole PDF, and nothing is left beside it.
pub async fn export_with(
    steps: &impl ExportSteps,
    app_url: &Url,
    course_id: Uuid,
    suggested_name: &str,
    timeouts: Timeouts,
) -> PdfOutcome {
    let file_name = suggested_file_name(suggested_name);
    let Some(destination) = steps.choose_destination(&file_name).await else {
        return PdfOutcome::Cancelled;
    };
    let mut states = match steps.open_view(&export_url(app_url, course_id)) {
        Ok(states) => states,
        Err(reason) => return PdfOutcome::Failed(reason),
    };
    let outcome = print_when_ready(steps, &mut states, destination, timeouts).await;
    steps.close_view();
    outcome
}

async fn print_when_ready(
    steps: &impl ExportSteps,
    states: &mut UnboundedReceiver<ViewState>,
    destination: PathBuf,
    timeouts: Timeouts,
) -> PdfOutcome {
    let failure = match tokio::time::timeout(timeouts.ready, states.recv()).await {
        Ok(Some(ViewState::Ready)) => None,
        Ok(Some(ViewState::Failed)) => Some("the export view could not read the course".to_owned()),
        Ok(None) => Some("the export view closed before it was ready to print".to_owned()),
        Err(_) => Some(format!(
            "the export view was not ready to print within {:?}",
            timeouts.ready
        )),
    };
    if let Some(reason) = failure {
        return PdfOutcome::Failed(reason);
    }
    let pages = pages_path();
    let printed =
        match tokio::time::timeout(timeouts.print, steps.print(&print_settings(&pages))).await {
            Ok(printed) => printed.and_then(|()| move_onto(&pages, &destination)),
            Err(_) => {
                // So that the operation no longer runs, nor stops the next export.
                steps.cancel_print();
                Err(format!(
                    "the print operation did not finish within {:?}",
                    timeouts.print
                ))
            }
        };
    // Moved, never started, or partly written.
    let _ = std::fs::remove_file(&pages);
    match printed {
        Ok(()) => PdfOutcome::Saved(destination),
        Err(reason) => PdfOutcome::Failed(reason),
    }
}

/// A file of one export's own in the temporary folder, which the pages are
/// printed to: a print that ends after its export gave up writes there.
fn pages_path() -> PathBuf {
    std::env::temp_dir().join(format!("atlas-export-{}.pdf", Uuid::new_v4()))
}

/// Moves the printed `pages` onto `destination`, copying them over it when a
/// move cannot (another volume, or a folder where only the file is writable).
fn move_onto(pages: &Path, destination: &Path) -> Result<(), String> {
    std::fs::rename(pages, destination)
        .or_else(|_| std::fs::copy(pages, destination).map(drop))
        .map_err(|error| {
            format!(
                "the PDF could not be saved to {}: {error}",
                destination.display()
            )
        })
}

/// The real exporter: the dialog plugin's save dialog, a hidden window of
/// the app, and WebKit's print operation.
pub struct WebviewPdfExporter {
    app: AppHandle,
    /// Where the app is served: the bundled interface or the development
    /// server.
    app_url: Url,
}

impl WebviewPdfExporter {
    pub fn new(app: AppHandle, app_url: Url) -> Self {
        Self { app, app_url }
    }
}

#[async_trait]
impl PdfExporter for WebviewPdfExporter {
    async fn export(&self, course_id: Uuid, suggested_name: &str) -> PdfOutcome {
        export_with(self, &self.app_url, course_id, suggested_name, TIMEOUTS).await
    }
}

impl ExportSteps for WebviewPdfExporter {
    async fn choose_destination(&self, file_name: &str) -> Option<PathBuf> {
        let (chosen, answer) = oneshot::channel();
        let mut dialog = self
            .app
            .dialog()
            .file()
            .set_file_name(file_name)
            .add_filter("PDF document", &["pdf"])
            .set_can_create_directories(true);
        if let Some(main) = self.app.get_webview_window("main") {
            dialog = dialog.set_parent(&main);
        }
        // The dialog asks before replacing an existing file.
        dialog.save_file(move |path| {
            let _ = chosen.send(path);
        });
        answer.await.ok().flatten()?.into_path().ok()
    }

    fn open_view(&self, url: &Url) -> Result<UnboundedReceiver<ViewState>, String> {
        let (report, states) = unbounded_channel();
        WebviewWindowBuilder::new(&self.app, EXPORT_WINDOW, WebviewUrl::External(url.clone()))
            .title("Atlas export")
            .visible(false)
            .focused(false)
            .skip_taskbar(true)
            .inner_size(VIEW_SIZE.0, VIEW_SIZE.1)
            // A hidden page still draws its diagrams and loads its fonts.
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .initialization_script(STATE_SCRIPT)
            .on_document_title_changed(move |_, title| {
                if let Some(state) = view_state(&title) {
                    let _ = report.send(state);
                }
            })
            .build()
            .map_err(|error| format!("the export view could not be opened: {error}"))?;
        Ok(states)
    }

    async fn print(&self, settings: &PrintSettings) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(EXPORT_WINDOW)
            .ok_or_else(|| "the export view closed before it was printed".to_owned())?;
        let (done, finished) = oneshot::channel();
        let settings = settings.clone();
        window
            .with_webview(move |webview| print_operation::start(&webview, &settings, done))
            .map_err(|error| format!("the export view could not be printed: {error}"))?;
        finished
            .await
            .unwrap_or_else(|_| Err("the print operation ended without a result".to_owned()))
    }

    fn cancel_print(&self) {
        if let Err(error) = self.app.run_on_main_thread(print_operation::cancel) {
            eprintln!("could not cancel the print operation: {error}");
        }
    }

    fn close_view(&self) {
        if let Some(window) = self.app.get_webview_window(EXPORT_WINDOW)
            && let Err(error) = window.destroy()
        {
            eprintln!("could not close the export view: {error}");
        }
    }
}

/// WebKit's print operation, run on the main thread.
mod print_operation {
    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::ptr::null_mut;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool, NSObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSPrintCancelJob, NSPrintInfo, NSPrintJobSavingURL, NSPrintOperation, NSPrintSaveJob,
        NSPrintingPaginationMode, NSWindow,
    };
    use objc2_foundation::{NSSize, NSString, NSURL};
    use objc2_web_kit::WKWebView;
    use tauri::webview::PlatformWebview;
    use tokio::sync::oneshot;

    use super::{Pagination, PrintSettings};

    type Done = oneshot::Sender<Result<(), String>>;

    struct Ivars {
        done: Cell<Option<Done>>,
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and the class
        // does not implement `Drop`.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "AtlasPdfPrintDelegate"]
        #[ivars = Ivars]
        struct PrintDelegate;

        impl PrintDelegate {
            #[unsafe(method(printOperationDidRun:success:contextInfo:))]
            fn did_run(&self, operation: &NSPrintOperation, success: Bool, _context: *mut c_void) {
                if let Some(done) = self.ivars().done.take() {
                    let _ = done.send(if success.as_bool() {
                        Ok(())
                    } else {
                        Err("the print operation did not write the PDF".to_owned())
                    });
                }
                CURRENT.with_borrow_mut(|current| {
                    if current.as_deref().is_some_and(|current| std::ptr::eq(current, operation)) {
                        *current = None;
                    }
                });
                // The operation does not retain its delegate, so each is kept
                // until here and released once this call returns.
                let this = DELEGATES.with_borrow_mut(|delegates| {
                    let index = delegates
                        .iter()
                        .position(|delegate| std::ptr::eq(&**delegate, self))?;
                    Some(delegates.swap_remove(index))
                });
                if let Some(this) = this {
                    let _ = Retained::autorelease_ptr(this);
                }
            }
        }
    );

    impl PrintDelegate {
        fn new(mtm: MainThreadMarker, done: Done) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars {
                done: Cell::new(Some(done)),
            });
            // SAFETY: NSObject's designated initialiser.
            unsafe { msg_send![super(this), init] }
        }
    }

    thread_local! {
        /// The print operation an export is waiting for, if any.
        static CURRENT: RefCell<Option<Retained<NSPrintOperation>>> = const { RefCell::new(None) };
        /// The delegates of the operations that have not ended, cancelled
        /// ones included.
        static DELEGATES: RefCell<Vec<Retained<PrintDelegate>>> = const { RefCell::new(Vec::new()) };
    }

    /// Cancels the operation the export stopped waiting for. It keeps its
    /// delegate until it ends, and no longer stops the next export.
    pub(super) fn cancel() {
        if let Some(operation) = CURRENT.take() {
            // SAFETY: AppKit's constant, which cancels a running job.
            unsafe { operation.printInfo().setJobDisposition(NSPrintCancelJob) };
        }
    }

    /// Starts printing `webview` with `settings`; `done` receives the end.
    pub(super) fn start(webview: &PlatformWebview, settings: &PrintSettings, done: Done) {
        let Some(mtm) = MainThreadMarker::new() else {
            let _ = done.send(Err(
                "the print operation must start on the main thread".into()
            ));
            return;
        };
        if CURRENT.with_borrow(Option::is_some) {
            let _ = done.send(Err("another export is still printing".into()));
            return;
        }
        let info = match print_info(settings) {
            Ok(info) => info,
            Err(reason) => {
                let _ = done.send(Err(reason));
                return;
            }
        };
        // SAFETY: on macOS the platform webview is a WKWebView in an
        // NSWindow, both alive for the duration of this call on the main
        // thread.
        let (web_view, window) = unsafe {
            (
                &*webview.inner().cast::<WKWebView>(),
                &*webview.ns_window().cast::<NSWindow>(),
            )
        };
        // SAFETY: `info` is a valid print info.
        let operation = unsafe { web_view.printOperationWithPrintInfo(&info) };
        operation.setShowsPrintPanel(settings.shows_print_panel);
        operation.setShowsProgressPanel(settings.shows_progress_panel);
        // WebKit lays the pages out in the operation's view, which starts
        // with no size.
        if let Some(view) = operation.view() {
            view.setFrame(web_view.frame());
        }
        let delegate = PrintDelegate::new(mtm, done);
        CURRENT.set(Some(operation.clone()));
        DELEGATES.with_borrow_mut(|delegates| delegates.push(delegate.clone()));
        let delegate: &AnyObject = &delegate;
        // SAFETY: the delegate answers the selector with the signature the
        // operation calls, and is kept alive until it is called; no context
        // is passed.
        unsafe {
            operation.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                window,
                Some(delegate),
                Some(sel!(printOperationDidRun:success:contextInfo:)),
                null_mut(),
            );
        }
    }

    fn print_info(settings: &PrintSettings) -> Result<Retained<NSPrintInfo>, String> {
        let destination = NSURL::from_file_path(&settings.destination).ok_or_else(|| {
            format!(
                "{} is not a file the PDF can be written to",
                settings.destination.display()
            )
        })?;
        let info = NSPrintInfo::new();
        info.setPaperName(Some(&NSString::from_str(settings.paper.name)));
        info.setPaperSize(NSSize::new(settings.paper.width, settings.paper.height));
        info.setTopMargin(settings.margins.top);
        info.setRightMargin(settings.margins.right);
        info.setBottomMargin(settings.margins.bottom);
        info.setLeftMargin(settings.margins.left);
        info.setHorizontalPagination(mode(settings.horizontal_pagination));
        info.setVerticalPagination(mode(settings.vertical_pagination));
        info.setHorizontallyCentered(false);
        info.setVerticallyCentered(false);
        // SAFETY: AppKit's constants, and the saving URL is an NSURL as the
        // print info's dictionary expects.
        unsafe {
            info.setJobDisposition(NSPrintSaveJob);
            info.dictionary().insert(NSPrintJobSavingURL, &destination);
        }
        Ok(info)
    }

    fn mode(pagination: Pagination) -> NSPrintingPaginationMode {
        match pagination {
            Pagination::Automatic => NSPrintingPaginationMode::Automatic,
            Pagination::Fit => NSPrintingPaginationMode::Fit,
        }
    }
}
