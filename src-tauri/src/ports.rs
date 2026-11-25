//! The core's ports on the desktop: the system browser and the events the
//! interface receives. The secret store is in [`crate::secrets`].

use std::sync::mpsc::{Sender, channel};

use atlas_core::ports::{Browser, BrowserError, EventSink};
use atlas_core::protocol::CoreEvent;
use tauri::{AppHandle, Url};
use tauri_plugin_opener::OpenerExt;

/// The system browser, through the opener plugin. Only the core calls it;
/// the webview holds no opener permission.
pub struct SystemBrowser(pub AppHandle);

impl Browser for SystemBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        self.0
            .opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|error| BrowserError(error.to_string()))
    }
}

/// Passes the core's events on one at a time, in the order the core emits
/// them, from a thread of its own, so that no core thread waits on the
/// interface. Responses go back over the command instead, so one can reach
/// the interface before the events the core emitted just ahead of it; the
/// interface's store drops a generation's events that arrive after the
/// response that settled it.
pub struct ForwardedEvents(Sender<CoreEvent>);

impl ForwardedEvents {
    /// Starts the forwarding thread; it ends when the core drops its sink.
    pub fn start(deliver: impl Fn(CoreEvent) + Send + 'static) -> Self {
        let (sender, receiver) = channel();
        std::thread::Builder::new()
            .name("core-events".to_owned())
            .spawn(move || {
                for event in receiver {
                    deliver(event);
                }
            })
            .expect("the core-events thread starts");
        Self(sender)
    }
}

impl EventSink for ForwardedEvents {
    fn emit(&self, event: CoreEvent) {
        // The thread ends only with the app, after the core.
        let _ = self.0.send(event);
    }
}
