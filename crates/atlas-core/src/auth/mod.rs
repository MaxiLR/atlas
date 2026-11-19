//! Sign in with ChatGPT: PKCE with a callback on `127.0.0.1`, the issued
//! client id, token exchange, refresh and revocation (ADR-0002).
//!
//! Tokens live only in the secret store and inside the core: none of these
//! types serialises, and none is reachable from the protocol.

use std::fmt;

use chrono::{DateTime, Utc};

/// A bearer token for the plan route. Its `Debug` output hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }

    /// The token itself, for an `Authorization` header.
    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken(..)")
    }
}

/// Exchanged for fresh tokens. Its `Debug` output hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct RefreshToken(String);

impl RefreshToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }

    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RefreshToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RefreshToken(..)")
    }
}

/// The plan login's tokens; rotated together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTokens {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
    pub expires_at: DateTime<Utc>,
}

/// Who signed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub email: String,
}

/// The installation's host id, a `urn:uuid:` generated once and reused for
/// every sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostId(pub String);
