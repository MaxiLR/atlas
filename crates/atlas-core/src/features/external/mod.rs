//! Links opened in the system browser: ChatGPT's usage settings, and `http`
//! or `https` links from the interface.

use std::sync::Arc;

use url::Url;

use super::Features;
use crate::CoreDeps;
use crate::ports::{Browser, BrowserError};
use crate::protocol::CoreError;
use crate::store::Store;

const MANAGE_USAGE: &str = "https://chatgpt.com/settings/usage";

pub(crate) struct Feature {
    browser: Arc<dyn Browser>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            browser: deps.browser.clone(),
        })
    }

    pub(crate) async fn open_manage_usage(&self, _cx: &Features) -> Result<(), CoreError> {
        let url = Url::parse(MANAGE_USAGE).expect("a valid usage settings URL");
        self.browser.open(&url).map_err(browser_error)
    }

    /// Opens `url` only when it is an `http` or `https` link.
    pub(crate) async fn open_external(&self, _cx: &Features, url: String) -> Result<(), CoreError> {
        let parsed = Url::parse(&url).map_err(|_| CoreError::InvalidRequest {
            message: format!("{url} is not a link"),
        })?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(CoreError::InvalidRequest {
                message: format!(
                    "only web links open in the browser, not {}: links",
                    parsed.scheme()
                ),
            });
        }
        self.browser.open(&parsed).map_err(browser_error)
    }
}

fn browser_error(error: BrowserError) -> CoreError {
    CoreError::Provider {
        message: error.to_string(),
    }
}
