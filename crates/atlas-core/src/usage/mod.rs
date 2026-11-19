//! The usage reader, a port of Headroom's Codex provider (ADR-0007): its own
//! device-code sign-in, refresh, and `GET {chatgpt}/backend-api/wham/usage`.
//! It shares nothing with the plan login but the secret store.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use url::Url;

use crate::auth::{AccessToken, RefreshToken};
use crate::ports::{Browser, Clock, Endpoints};
use crate::protocol::{UsageFailure, UsageLevel};

/// The device code shown while the usage login waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_url: Url,
    pub expires_at: DateTime<Utc>,
}

/// Who signed in to the usage meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAccount {
    pub account_id: String,
    pub email: Option<String>,
    /// The plan as words: `Plus`, `Self serve business prolite`.
    pub plan: Option<String>,
}

/// The usage meter's login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageLogin {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
    pub expires_at: DateTime<Utc>,
    pub signed_in_at: DateTime<Utc>,
    pub account: UsageAccount,
}

/// One usage window as the account reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub id: String,
    pub label: String,
    pub length_seconds: u64,
    pub used: f64,
    pub resets_at: Option<DateTime<Utc>>,
    pub at_limit: bool,
}

/// One reading.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub windows: Vec<Window>,
    pub plan: Option<String>,
}

pub struct UsageReaderDeps {
    pub endpoints: Endpoints,
    pub clock: Arc<dyn Clock>,
    pub browser: Arc<dyn Browser>,
}

pub struct UsageReader {
    _deps: UsageReaderDeps,
}

impl UsageReader {
    pub fn new(deps: UsageReaderDeps) -> Self {
        Self { _deps: deps }
    }

    pub async fn login(
        &self,
        _present: impl FnOnce(DeviceCode) + Send,
        _cancelled: impl Fn() -> bool + Send + Sync,
    ) -> Result<UsageLogin, UsageFailure> {
        todo!("the usage login")
    }

    pub async fn refresh(&self, _login: &UsageLogin) -> Result<UsageLogin, UsageFailure> {
        todo!("the usage refresh")
    }

    pub async fn read(&self, _login: &UsageLogin) -> Result<Usage, UsageFailure> {
        todo!("the usage reading")
    }
}

pub fn level_of(_window: &Window) -> UsageLevel {
    todo!("the level of a window")
}
