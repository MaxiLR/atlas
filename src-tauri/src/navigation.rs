//! The webview's navigation rule: it shows the app and nothing else. Links
//! open in the system browser through the core, never in the window.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Runtime, Url};

/// Whether `url` is the app itself: the bundled interface on Tauri's
/// `tauri://localhost`, or, given `dev_url`, a page of the development
/// server that serves it.
pub fn is_app_url(url: &Url, dev_url: Option<&Url>) -> bool {
    let bundled = url.scheme() == "tauri" && url.host_str() == Some("localhost");
    let development = dev_url.is_some_and(|dev_url| {
        let origin = url.origin();
        origin.is_tuple() && origin == dev_url.origin()
    });
    bundled || development
}

/// Refuses every navigation of a webview to a URL that is not the app;
/// `dev_url` is the development server while one serves the app.
pub fn plugin<R: Runtime>(dev_url: Option<Url>) -> TauriPlugin<R> {
    Builder::new("navigation")
        .on_navigation(move |_webview, url| is_app_url(url, dev_url.as_ref()))
        .build()
}
