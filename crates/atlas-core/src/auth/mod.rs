//! Sign in with ChatGPT: PKCE with a callback on `127.0.0.1`, the issued
//! client id, token exchange, refresh and revocation (ADR-0002).
//!
//! Tokens and the issued client id live only in the secret store and inside
//! the core: they are serialised only into the secret store's records, and
//! none is reachable from the protocol.

mod callback;
mod flow;
mod host_id;
mod id_token;
mod oauth;
mod refresh;
mod sign_out;
mod stored;

pub use flow::PendingSignIn;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::ports::{Browser, BrowserError, Clock, Endpoints, SecretError, SecretStore};
use stored::StoredLogin;

/// The lifetime assumed for an access token whose answer names none.
const DEFAULT_LIFETIME_SECONDS: i64 = 3600;
/// How long a request to the authorisation server may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    /// The provider reported that the account or workspace may not use the
    /// plan. Kept with the login until a new sign-in replaces it.
    pub plan_not_allowed: bool,
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

/// The plan login's OAuth mechanics.
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
    host_id: HostId,
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
                host_id: deps.host_id,
                refresh_lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    /// Opens the browser on the authorisation page with a listener waiting
    /// for its callback on `127.0.0.1`.
    pub async fn start(&self, mode: SignInMode) -> Result<PendingSignIn, AuthError> {
        flow::start(self.inner.clone(), mode).await
    }

    /// The saved login, without its tokens.
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

    /// Revokes the refresh token, then clears the tokens.
    pub async fn sign_out(&self) -> Result<SignedOut, AuthError> {
        sign_out::sign_out(&self.inner).await
    }
}
