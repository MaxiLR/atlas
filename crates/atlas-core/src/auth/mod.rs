//! Sign in with ChatGPT through OpenAI's Codex OAuth client: PKCE with the
//! registered loopback callback, the code exchange and refresh (ADR-0002).
//!
//! The tokens live only in the secret store and inside the core: they are
//! serialised only into the secret store's record, and none is reachable
//! from the protocol. A login saved by the earlier plan-sharing sign-in is
//! not a Codex login: it reads as no login, so the candidate signs in again.

mod callback;
mod claims;
mod flow;
mod oauth;
mod refresh;
mod stored;

pub use flow::PendingSignIn;

pub(crate) use claims::token_claims;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::ports::{Browser, BrowserError, Clock, Endpoints, SecretError, SecretStore};
use stored::StoredLogin;

/// OpenAI's Codex OAuth client, the only one Atlas signs in with.
pub const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// The port of the only callback OpenAI allowlists for that client.
pub const CALLBACK_PORT: u16 = 1455;

/// The lifetime assumed for an access token whose answer names none.
const DEFAULT_LIFETIME_SECONDS: i64 = 3600;
/// How long a request to the authorisation server may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A bearer token for the Codex backend. Its `Debug` output hides it.
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

/// Who signed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub email: String,
}

/// What the sign-in needs from its host.
pub struct PlanAuthDeps {
    pub endpoints: Endpoints,
    pub secrets: Arc<dyn SecretStore>,
    pub browser: Arc<dyn Browser>,
    pub clock: Arc<dyn Clock>,
}

/// The Codex login as the core sees it, without its tokens. A Codex login
/// always spends the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub identity: Identity,
    /// When the access token expires.
    pub expires_at: DateTime<Utc>,
    /// The provider reported that the account or workspace may not use the
    /// plan. Kept with the login until a new sign-in replaces it.
    pub plan_not_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("permission was declined")]
    Declined,
    #[error("the callback's state does not match this attempt")]
    StateMismatch,
    #[error("the authorisation server refused: {error}")]
    Authorization { error: String },
    /// The registered callback cannot listen: another program holds the port
    /// or the system refused it. Nothing is stopped to free it.
    #[error("the sign-in cannot listen on port {port} for its callback: {reason}")]
    CallbackUnavailable { port: u16, reason: String },
    #[error("the code exchange failed: {0}")]
    TokenEndpoint(String),
    #[error("the ID token was rejected: {0}")]
    IdTokenRejected(String),
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
    Unavailable(Unreachable),
    #[error("the token endpoint refused the refresh ({status})")]
    Refused { status: u16, code: Option<String> },
    #[error(transparent)]
    Secret(#[from] SecretError),
}

/// Why the token endpoint could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unreachable {
    /// No answer: the network or the connection failed.
    #[error("{0}")]
    Network(String),
    /// A server error or an answer that could not be read.
    #[error("{0}")]
    Answer(String),
}

/// The Codex login's OAuth mechanics.
#[derive(Clone)]
pub struct PlanAuth {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    endpoints: Endpoints,
    secrets: Arc<dyn SecretStore>,
    browser: Arc<dyn Browser>,
    clock: Arc<dyn Clock>,
    /// Held while the tokens are refreshed, replaced or cleared.
    refresh_lock: tokio::sync::Mutex<()>,
}

impl Inner {
    fn expires_at(&self, expires_in: Option<i64>) -> DateTime<Utc> {
        self.clock.now() + chrono::Duration::seconds(expires_in.unwrap_or(DEFAULT_LIFETIME_SECONDS))
    }
}

impl PlanAuth {
    pub fn new(deps: PlanAuthDeps) -> Self {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("the HTTP client builds");
        Self {
            inner: Arc::new(Inner {
                http,
                endpoints: deps.endpoints,
                secrets: deps.secrets,
                browser: deps.browser,
                clock: deps.clock,
                refresh_lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// Listens on the registered callback port, then opens the browser on
    /// the authorisation page.
    pub async fn start(&self) -> Result<PendingSignIn, AuthError> {
        flow::start(self.inner.clone()).await
    }

    /// The saved Codex login, without its tokens.
    pub fn login(&self) -> Result<Option<Login>, AuthError> {
        Ok(StoredLogin::read(&*self.inner.secrets)?.map(|login| login.login()))
    }

    /// The saved access token, for the core's token source.
    pub fn access_token(&self) -> Result<Option<AccessToken>, AuthError> {
        Ok(StoredLogin::read(&*self.inner.secrets)?.map(|login| login.access_token()))
    }

    /// Exchanges the refresh token for new tokens. Concurrent callers share
    /// one request.
    pub async fn refresh(&self) -> Result<AccessToken, RefreshError> {
        refresh::refresh(&self.inner).await
    }

    /// Remembers with the saved login that the provider reported the account
    /// or workspace not allowed to use the plan. Answers whether there was a
    /// login to mark.
    pub async fn mark_plan_not_allowed(&self) -> Result<bool, AuthError> {
        let _serialised = self.inner.refresh_lock.lock().await;
        let Some(mut login) = StoredLogin::read(&*self.inner.secrets)? else {
            return Ok(false);
        };
        if !login.plan_not_allowed {
            login.plan_not_allowed = true;
            login.write(&*self.inner.secrets)?;
        }
        Ok(true)
    }

    /// Clears the login locally, a plan-sharing record included. The Codex
    /// client has no revocation to call.
    pub async fn sign_out(&self) -> Result<(), AuthError> {
        let _serialised = self.inner.refresh_lock.lock().await;
        StoredLogin::clear(&*self.inner.secrets)?;
        Ok(())
    }
}
