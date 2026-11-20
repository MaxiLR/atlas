//! In-memory fakes for every port and for the model provider. Tests compose
//! the core from these; nothing here touches the network, the Keychain or the
//! app's data folder.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use url::Url;
use uuid::Uuid;

use crate::CoreDeps;
use crate::auth::AccessToken;
use crate::ports::{
    Browser, BrowserError, Clock, Endpoints, EventSink, PdfExporter, PdfOutcome, SecretError,
    SecretRecord, SecretStore,
};
use crate::protocol::CoreEvent;
pub use crate::provider::{FakeProvider, FakeScript};
use crate::provider::{ModelProvider, ProviderError, TokenSource};

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().expect("fake lock poisoned")
}

/// A clock that moves only when told to. `sleep` records the pause and moves
/// the clock by it at once.
#[derive(Debug)]
pub struct FakeClock {
    now: Mutex<DateTime<Utc>>,
    sleeps: Mutex<Vec<Duration>>,
}

impl FakeClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
            sleeps: Mutex::new(Vec::new()),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut now = lock(&self.now);
        *now += chrono::Duration::from_std(by).expect("a representable duration");
    }

    /// The pauses asked for so far, in order.
    pub fn sleeps(&self) -> Vec<Duration> {
        lock(&self.sleeps).clone()
    }
}

impl Default for FakeClock {
    /// 2026-01-01T00:00:00Z.
    fn default() -> Self {
        Self::new(
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
                .single()
                .expect("a valid date"),
        )
    }
}

#[async_trait]
impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *lock(&self.now)
    }

    async fn sleep(&self, duration: Duration) {
        lock(&self.sleeps).push(duration);
        self.advance(duration);
    }
}

#[derive(Debug, Default)]
pub struct MemorySecretStore {
    records: Mutex<HashMap<SecretRecord, String>>,
}

impl MemorySecretStore {
    pub fn get(&self, record: SecretRecord) -> Option<String> {
        lock(&self.records).get(&record).cloned()
    }
}

impl SecretStore for MemorySecretStore {
    fn read(&self, record: SecretRecord) -> Result<Option<String>, SecretError> {
        Ok(self.get(record))
    }

    fn write(&self, record: SecretRecord, value: &str) -> Result<(), SecretError> {
        lock(&self.records).insert(record, value.to_owned());
        Ok(())
    }

    fn delete(&self, record: SecretRecord) -> Result<(), SecretError> {
        lock(&self.records).remove(&record);
        Ok(())
    }
}

/// Records every URL it is asked to open and opens none.
#[derive(Debug, Default)]
pub struct RecordingBrowser {
    opened: Mutex<Vec<Url>>,
}

impl RecordingBrowser {
    pub fn opened(&self) -> Vec<Url> {
        lock(&self.opened).clone()
    }
}

impl Browser for RecordingBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        lock(&self.opened).push(url.clone());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct RecordingEvents {
    events: Mutex<Vec<CoreEvent>>,
}

impl RecordingEvents {
    /// The events emitted since the last call, in order.
    pub fn take(&self) -> Vec<CoreEvent> {
        std::mem::take(&mut lock(&self.events))
    }
}

impl EventSink for RecordingEvents {
    fn emit(&self, event: CoreEvent) {
        lock(&self.events).push(event);
    }
}

/// Answers every export with a scripted outcome and records what it was asked.
#[derive(Debug)]
pub struct FakePdfExporter {
    outcome: Mutex<PdfOutcome>,
    requests: Mutex<Vec<(Uuid, String)>>,
}

impl FakePdfExporter {
    pub fn answering(outcome: PdfOutcome) -> Self {
        Self {
            outcome: Mutex::new(outcome),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Each export's course and suggested file name, in order.
    pub fn requests(&self) -> Vec<(Uuid, String)> {
        lock(&self.requests).clone()
    }
}

impl Default for FakePdfExporter {
    fn default() -> Self {
        Self::answering(PdfOutcome::Cancelled)
    }
}

#[async_trait]
impl PdfExporter for FakePdfExporter {
    async fn export(&self, course_id: Uuid, suggested_name: &str) -> PdfOutcome {
        lock(&self.requests).push((course_id, suggested_name.to_owned()));
        lock(&self.outcome).clone()
    }
}

/// Hands out `token`, then `token-1`, `token-2`… on each refresh, and records
/// whether the login was reported rejected.
#[derive(Debug)]
pub struct FakeTokenSource {
    token: String,
    refreshes: AtomicUsize,
    rejected: Mutex<bool>,
}

impl FakeTokenSource {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_owned(),
            refreshes: AtomicUsize::new(0),
            rejected: Mutex::new(false),
        }
    }

    pub fn refreshes(&self) -> usize {
        self.refreshes.load(Ordering::SeqCst)
    }

    pub fn was_rejected(&self) -> bool {
        *lock(&self.rejected)
    }

    fn current(&self) -> AccessToken {
        match self.refreshes() {
            0 => AccessToken::new(self.token.clone()),
            n => AccessToken::new(format!("{}-{n}", self.token)),
        }
    }
}

#[async_trait]
impl TokenSource for FakeTokenSource {
    async fn access_token(&self) -> Result<AccessToken, ProviderError> {
        Ok(self.current())
    }

    async fn refresh(&self) -> Result<AccessToken, ProviderError> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        Ok(self.current())
    }

    async fn rejected(&self) {
        *lock(&self.rejected) = true;
    }
}

/// The host's ports as fakes, kept so a test can inspect them after building
/// the core from them.
#[derive(Debug)]
pub struct FakePorts {
    pub clock: Arc<FakeClock>,
    pub secrets: Arc<MemorySecretStore>,
    pub browser: Arc<RecordingBrowser>,
    pub pdf: Arc<FakePdfExporter>,
    /// By default a closed local port: any request fails to connect.
    pub endpoints: Endpoints,
}

impl Default for FakePorts {
    fn default() -> Self {
        Self {
            clock: Arc::default(),
            secrets: Arc::default(),
            browser: Arc::default(),
            pdf: Arc::default(),
            endpoints: Endpoints::local(
                &Url::parse("http://127.0.0.1:9/").expect("a valid local URL"),
            ),
        }
    }
}

impl FakePorts {
    /// The core's dependencies: these ports, the given database file, model
    /// provider and event sink.
    pub fn deps(
        &self,
        database_path: PathBuf,
        provider: Arc<dyn ModelProvider>,
        events: Arc<dyn EventSink>,
    ) -> CoreDeps {
        CoreDeps {
            database_path,
            provider,
            events,
            clock: self.clock.clone(),
            secrets: self.secrets.clone(),
            browser: self.browser.clone(),
            pdf: self.pdf.clone(),
            endpoints: self.endpoints.clone(),
        }
    }
}
