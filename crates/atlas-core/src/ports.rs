//! What the core needs from its host, injected through [`crate::CoreDeps`].
//! Tests use the in-memory fakes in [`crate::testing`].

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use url::Url;
use uuid::Uuid;

use crate::protocol::CoreEvent;

/// Receives the core's events; the shell forwards them to the interface.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: CoreEvent);
}

/// The time, and the pauses between retries and readings.
#[async_trait]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    async fn sleep(&self, duration: Duration);
}

/// The wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

#[async_trait]
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// The records the secret store holds, one item each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretRecord {
    /// The plan login's tokens.
    PlanLogin,
    /// The client id issued at the first sign-in, kept across sign-outs.
    IssuedClientId,
    /// The usage meter's own login.
    UsageLogin,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the secret store failed: {0}")]
pub struct SecretError(pub String);

/// Where tokens live: the macOS Keychain in the app. Values are opaque
/// strings encoded by their owner and never logged.
pub trait SecretStore: Send + Sync {
    fn read(&self, record: SecretRecord) -> Result<Option<String>, SecretError>;
    fn write(&self, record: SecretRecord, value: &str) -> Result<(), SecretError>;
    /// Deleting a record that is absent succeeds.
    fn delete(&self, record: SecretRecord) -> Result<(), SecretError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the browser could not open the link: {0}")]
pub struct BrowserError(pub String);

/// The system browser.
pub trait Browser: Send + Sync {
    fn open(&self, url: &Url) -> Result<(), BrowserError>;
}

/// How a PDF export ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfOutcome {
    Saved(PathBuf),
    /// The candidate dismissed the save dialog.
    Cancelled,
    Failed(String),
}

/// Prints a course's export view to a file the candidate chooses.
#[async_trait]
pub trait PdfExporter: Send + Sync {
    async fn export(&self, course_id: Uuid, suggested_name: &str) -> PdfOutcome;
}

/// The base URLs of every service the core calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// The Responses API: `{api}/responses`, `{api}/models`.
    pub api: Url,
    /// The authorisation server of Sign in with ChatGPT.
    pub auth: Url,
    /// ChatGPT, for the usage reading.
    pub chatgpt: Url,
    /// Wikipedia's REST API.
    pub wikipedia: Url,
}

impl Endpoints {
    pub fn production() -> Self {
        let parse = |url: &str| Url::parse(url).expect("a valid production URL");
        Self {
            api: parse("https://api.openai.com/v1"),
            auth: parse("https://auth.openai.com"),
            chatgpt: parse("https://chatgpt.com"),
            wikipedia: parse("https://en.wikipedia.org/api/rest_v1"),
        }
    }

    /// Every service under its own path of one local server, for tests:
    /// `{base}api`, `{base}auth`, `{base}chatgpt` and `{base}wikipedia`.
    pub fn local(base: &Url) -> Self {
        let under = |path: &str| base.join(path).expect("a valid local URL");
        Self {
            api: under("api"),
            auth: under("auth"),
            chatgpt: under("chatgpt"),
            wikipedia: under("wikipedia"),
        }
    }
}
