//! Sign in with ChatGPT: PKCE with a callback on `127.0.0.1`, the issued
//! client id, token exchange, refresh and revocation (ADR-0002).
//!
//! Tokens live only in the secret store and inside the core: none of these
//! types serialises, and none is reachable from the protocol.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use url::Url;

use crate::ports::{Browser, BrowserError, Clock, Endpoints, SecretError, SecretStore};

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

impl HostId {
    /// The host id kept in the file at `path`, generated and written there the
    /// first time.
    pub fn load_or_create(_path: &Path) -> std::io::Result<Self> {
        unimplemented!("host id")
    }
}

/// What the sign-in needs from its host.
pub struct PlanAuthDeps {
    pub endpoints: Endpoints,
    pub secrets: Arc<dyn SecretStore>,
    pub browser: Arc<dyn Browser>,
    pub clock: Arc<dyn Clock>,
    pub host_id: HostId,
}

/// Which authorisation a sign-in attempt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInMode {
    /// A first registration, or a sign-in with the saved client id.
    SignIn,
    /// Ask again for the plan scope with the saved client id, forcing consent.
    EnablePlanUsage,
}

/// The plan login as the core sees it, without its tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub identity: Identity,
    /// The scopes the authorisation server granted.
    pub scopes: Vec<String>,
    /// Whether `chatgpt.tokens.use.direct` was granted.
    pub plan_usage: bool,
    /// When the access token expires.
    pub expires_at: DateTime<Utc>,
}

/// Whether the authorisation server confirmed the end of the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revocation {
    Confirmed,
    /// The tokens were cleared locally but the server did not confirm.
    Unconfirmed {
        reason: String,
    },
    /// There was no login to revoke.
    NothingToRevoke,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOut {
    pub revocation: Revocation,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("permission was declined")]
    Declined,
    #[error("the callback's state does not match this attempt")]
    StateMismatch,
    #[error("the authorisation server refused: {error}")]
    Authorization { error: String },
    #[error("the registration returned no issued client id")]
    RegistrationIncomplete,
    #[error("the callback named a different client id")]
    ClientMismatch,
    #[error("there is no registration to reauthorise")]
    NotRegistered,
    #[error("the code exchange failed: {0}")]
    TokenEndpoint(String),
    #[error("the ID token was rejected: {0}")]
    IdTokenRejected(String),
    #[error("the discovery document is unusable: {0}")]
    Discovery(String),
    #[error("the callback failed: {0}")]
    Callback(String),
    #[error(transparent)]
    Browser(#[from] BrowserError),
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error("the network failed: {0}")]
    Network(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RefreshError {
    #[error("there is no login to refresh")]
    SignedOut,
    /// A terminal refresh error: the tokens were cleared.
    #[error("the login ended: {code}")]
    LoginEnded { code: String },
    /// A network error or a server error: nothing was cleared.
    #[error("the token endpoint is unavailable: {0}")]
    Unavailable(String),
    #[error("the token endpoint refused the refresh ({status})")]
    Refused { status: u16, code: Option<String> },
    #[error(transparent)]
    Secret(#[from] SecretError),
}

/// The plan login's OAuth mechanics.
#[derive(Clone)]
pub struct PlanAuth {
    _deps: Arc<PlanAuthDeps>,
}

impl PlanAuth {
    pub fn new(deps: PlanAuthDeps) -> Self {
        Self {
            _deps: Arc::new(deps),
        }
    }

    /// Opens the browser on the authorisation page with a listener waiting
    /// for its callback.
    pub async fn start(&self, _mode: SignInMode) -> Result<PendingSignIn, AuthError> {
        unimplemented!("start")
    }

    pub fn login(&self) -> Result<Option<Login>, AuthError> {
        unimplemented!("login")
    }

    pub fn access_token(&self) -> Result<Option<AccessToken>, AuthError> {
        unimplemented!("access token")
    }

    pub async fn refresh(&self) -> Result<AccessToken, RefreshError> {
        unimplemented!("refresh")
    }

    pub async fn sign_out(&self) -> Result<SignedOut, AuthError> {
        unimplemented!("sign out")
    }
}

/// A sign-in attempt waiting for its callback. Dropping it cancels it.
pub struct PendingSignIn {
    redirect_uri: Url,
}

impl PendingSignIn {
    pub fn redirect_uri(&self) -> &Url {
        &self.redirect_uri
    }

    pub async fn finish(self) -> Result<Login, AuthError> {
        unimplemented!("finish")
    }
}
