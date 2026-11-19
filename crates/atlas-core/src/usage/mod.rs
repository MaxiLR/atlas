//! The usage reader, a port of Headroom's Codex provider (ADR-0007): its own
//! device-code sign-in, refresh, and `GET {chatgpt}/backend-api/wham/usage`.
//! It shares nothing with the plan login but the secret store.

use chrono::{DateTime, Utc};

use crate::auth::{AccessToken, RefreshToken};

/// The device code shown while the usage login waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_url: String,
    pub expires_at: DateTime<Utc>,
    pub interval_seconds: u32,
}

/// The usage meter's login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageLogin {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub plan: Option<String>,
}
