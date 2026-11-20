//! Sign in with ChatGPT through the Codex OAuth client, at the `PlanAuth`
//! boundary, against a fake authorisation server on `127.0.0.1`. The fake
//! browser follows the authorisation page's redirect to the callback on the
//! registered Codex port.
//!
//! The callback port is fixed, so each harness holds [`CALLBACK_PORT`] and
//! the tests run one at a time.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::auth::{AuthError, Login, PlanAuth, PlanAuthDeps, RefreshError, Unreachable};
use atlas_core::ports::{Browser, BrowserError, Clock, Endpoints, SecretRecord, SecretStore};
use atlas_core::testing::{FakeClock, MemorySecretStore};
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use url::Url;

const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const SUBJECT: &str = "user-7Qe2";
const EMAIL: &str = "ada@example.com";

const AUTHORIZE: &str = "/auth/oauth/authorize";
const TOKEN: &str = "/auth/oauth/token";

const TERMINAL_CODES: &[&str] = &[
    "invalid_grant",
    "invalid_refresh_token",
    "token_expired",
    "refresh_token_expired",
    "refresh_token_invalidated",
    "refresh_token_reused",
];

/// Held by every harness: sign-in listens on the one registered port.
static CALLBACK_PORT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// A JWT-shaped token: the client reads its claims without a signature.
fn jwt(claims: &Value) -> String {
    let part = |text: &str| URL_SAFE_NO_PAD.encode(text);
    format!(
        "{}.{}.{}",
        part(r#"{"alg":"RS256","typ":"JWT"}"#),
        part(&claims.to_string()),
        part("signature")
    )
}

/// One way to spoil the ID token the token endpoint returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flaw {
    WrongAudience,
    WrongIssuer,
    /// No email in the ID token; the access token's profile carries it.
    EmailOnlyInProfile,
}

// ---------------------------------------------------------------------------
// The fake authorisation server
// ---------------------------------------------------------------------------

/// How the server behaves; a test changes it between steps.
#[derive(Debug, Clone, Default)]
struct Knobs {
    /// The callback carries this OAuth error instead of a code.
    callback_error: Option<&'static str>,
    flaw: Option<Flaw>,
    /// The token endpoint answers refreshes with this status and code.
    refresh_refusal: Option<(u16, &'static str)>,
    /// The token endpoint holds each refresh this long.
    refresh_delay: Option<Duration>,
}

#[derive(Default)]
struct AuthState {
    knobs: Knobs,
    /// The form of every token request, in order.
    token_forms: Vec<HashMap<String, String>>,
    codes: HashSet<String>,
    counter: u32,
}

struct AuthServerInner {
    base: Url,
    clock: Arc<FakeClock>,
    state: Mutex<AuthState>,
}

impl AuthServerInner {
    fn state(&self) -> std::sync::MutexGuard<'_, AuthState> {
        self.state.lock().expect("server lock poisoned")
    }

    fn issuer(&self) -> String {
        format!("{}auth", self.base)
    }

    fn tokens(&self, flaw: Option<Flaw>, n: u32) -> Value {
        let now = self.clock.now().timestamp();
        let mut id = json!({
            "iss": if flaw == Some(Flaw::WrongIssuer) { "https://elsewhere.example".to_owned() } else { self.issuer() },
            "sub": SUBJECT,
            "aud": if flaw == Some(Flaw::WrongAudience) { "app_someone_else" } else { CODEX_CLIENT_ID },
            "iat": now,
            "exp": now + 3600,
        });
        if flaw != Some(Flaw::EmailOnlyInProfile) {
            id["email"] = json!(EMAIL);
        }
        json!({
            "access_token": jwt(&json!({
                "n": n,
                "https://api.openai.com/profile": { "email": EMAIL },
            })),
            "refresh_token": format!("refresh-{n}"),
            "id_token": jwt(&id),
            "token_type": "Bearer",
            "expires_in": 3600,
        })
    }
}

fn oauth_error(status: u16, error: &str) -> Response {
    (
        StatusCode::from_u16(status).expect("a valid status"),
        Json(json!({ "error": error })),
    )
        .into_response()
}

async fn authorize(
    State(server): State<Arc<AuthServerInner>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let param = |name: &str| params.get(name).cloned().unwrap_or_default();
    let mut callback = Url::parse(&param("redirect_uri")).expect("a redirect URI");
    let mut state = server.state();
    if let Some(error) = state.knobs.callback_error {
        callback
            .query_pairs_mut()
            .append_pair("error", error)
            .append_pair("state", &param("state"));
        return Redirect::to(callback.as_str()).into_response();
    }
    state.counter += 1;
    let code = format!("code-{}", state.counter);
    state.codes.insert(code.clone());
    callback
        .query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &param("state"));
    Redirect::to(callback.as_str()).into_response()
}

async fn token(
    State(server): State<Arc<AuthServerInner>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let field = |name: &str| form.get(name).cloned().unwrap_or_default();
    let knobs = {
        let mut state = server.state();
        state.token_forms.push(form.clone());
        state.knobs.clone()
    };
    if field("client_id") != CODEX_CLIENT_ID {
        return oauth_error(400, "invalid_client");
    }
    match field("grant_type").as_str() {
        "authorization_code" => {
            let mut state = server.state();
            if !state.codes.remove(&field("code")) {
                return oauth_error(400, "invalid_grant");
            }
            state.counter += 1;
            let n = state.counter;
            drop(state);
            Json(server.tokens(knobs.flaw, n)).into_response()
        }
        "refresh_token" => {
            if let Some(delay) = knobs.refresh_delay {
                tokio::time::sleep(delay).await;
            }
            if let Some((status, code)) = knobs.refresh_refusal {
                return oauth_error(status, code);
            }
            let n = {
                let mut state = server.state();
                state.counter += 1;
                state.counter
            };
            Json(server.tokens(None, n)).into_response()
        }
        _ => oauth_error(400, "unsupported_grant_type"),
    }
}

struct FakeAuthServer {
    inner: Arc<AuthServerInner>,
}

impl FakeAuthServer {
    async fn start(clock: Arc<FakeClock>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let inner = Arc::new(AuthServerInner {
            base: Url::parse(&format!("http://{address}/")).expect("a valid URL"),
            clock,
            state: Mutex::default(),
        });
        let app = Router::new()
            .route(AUTHORIZE, get(authorize))
            .route(TOKEN, post(token))
            .with_state(inner.clone());
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("the fake authorisation server runs");
        });
        Self { inner }
    }

    fn knobs(&self, change: impl FnOnce(&mut Knobs)) {
        change(&mut self.inner.state().knobs);
    }

    fn token_requests(&self, grant_type: &str) -> Vec<HashMap<String, String>> {
        self.inner
            .state()
            .token_forms
            .iter()
            .filter(|form| form.get("grant_type").map(String::as_str) == Some(grant_type))
            .cloned()
            .collect()
    }

    fn token_request_count(&self) -> usize {
        self.inner.state().token_forms.len()
    }
}

// ---------------------------------------------------------------------------
// The browser and the harness
// ---------------------------------------------------------------------------

/// Visits each page, following redirects, as the user's browser would after
/// the user approves.
struct RedirectingBrowser;

impl Browser for RedirectingBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        let url = url.clone();
        tokio::spawn(async move {
            let _ = reqwest::Client::new().get(url).send().await;
        });
        Ok(())
    }
}

struct Harness {
    server: FakeAuthServer,
    secrets: Arc<MemorySecretStore>,
    clock: Arc<FakeClock>,
    auth: PlanAuth,
    _port: tokio::sync::MutexGuard<'static, ()>,
}

impl Harness {
    async fn new() -> Self {
        let port = CALLBACK_PORT.lock().await;
        callback_port_released().await;
        Self::with_endpoints(port, None).await
    }

    async fn with_endpoints(
        port: tokio::sync::MutexGuard<'static, ()>,
        endpoints: Option<Endpoints>,
    ) -> Self {
        let secrets = Arc::new(MemorySecretStore::default());
        let clock = Arc::new(FakeClock::default());
        let server = FakeAuthServer::start(clock.clone()).await;
        let auth = PlanAuth::new(PlanAuthDeps {
            endpoints: endpoints.unwrap_or_else(|| Endpoints::local(&server.inner.base)),
            secrets: secrets.clone(),
            browser: Arc::new(RedirectingBrowser),
            clock: clock.clone(),
        });
        Self {
            server,
            secrets,
            clock,
            auth,
            _port: port,
        }
    }

    /// A harness after one successful sign-in.
    async fn signed_in() -> Self {
        let harness = Self::new().await;
        harness.sign_in().await.expect("the sign-in succeeds");
        harness
    }

    async fn sign_in(&self) -> Result<Login, AuthError> {
        let pending = self.auth.start().await?;
        tokio::time::timeout(Duration::from_secs(10), pending.finish())
            .await
            .expect("the sign-in ends")
    }

    fn access_token(&self) -> Option<String> {
        self.auth
            .access_token()
            .expect("the token reads")
            .map(|token| token.secret().to_owned())
    }
}

/// Waits a moment for an earlier test's sign-in to close its listener on the
/// callback port.
async fn callback_port_released() {
    for _ in 0..200 {
        if std::net::TcpListener::bind(("127.0.0.1", 1455)).is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// ---------------------------------------------------------------------------
// The callback and the ID token
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_completed_sign_in_saves_the_identity_from_the_id_token() {
    let harness = Harness::new().await;

    let login = harness.sign_in().await.expect("signed in");

    assert_eq!(login.identity.subject, SUBJECT);
    assert_eq!(login.identity.email, EMAIL);
    assert!(!login.plan_not_allowed);
    assert_eq!(
        login.expires_at,
        harness.clock.now() + chrono::Duration::seconds(3600)
    );
    assert_eq!(harness.auth.login().expect("the login reads"), Some(login));
    assert!(harness.access_token().is_some());
}

#[tokio::test]
async fn declined_permission_ends_the_attempt_without_an_exchange() {
    let harness = Harness::new().await;
    harness
        .server
        .knobs(|knobs| knobs.callback_error = Some("access_denied"));

    assert_eq!(harness.sign_in().await, Err(AuthError::Declined));

    assert_eq!(harness.server.token_request_count(), 0);
    assert_eq!(harness.auth.login(), Ok(None));
}

#[tokio::test]
async fn an_authorization_error_is_reported_without_an_exchange() {
    let harness = Harness::new().await;
    harness
        .server
        .knobs(|knobs| knobs.callback_error = Some("server_error"));

    assert_eq!(
        harness.sign_in().await,
        Err(AuthError::Authorization {
            error: "server_error".to_owned()
        })
    );
    assert_eq!(harness.server.token_request_count(), 0);
}

async fn assert_id_token_rejected(flaw: Flaw) {
    let harness = Harness::new().await;
    harness.server.knobs(|knobs| knobs.flaw = Some(flaw));

    let result = harness.sign_in().await;

    assert!(
        matches!(result, Err(AuthError::IdTokenRejected(_))),
        "{flaw:?} is rejected, got {result:?}"
    );
    assert_eq!(harness.auth.login(), Ok(None), "no login is saved");
}

#[tokio::test]
async fn an_id_token_for_another_client_is_rejected() {
    assert_id_token_rejected(Flaw::WrongAudience).await;
}

#[tokio::test]
async fn an_id_token_from_another_issuer_is_rejected() {
    assert_id_token_rejected(Flaw::WrongIssuer).await;
}

#[tokio::test]
async fn the_email_may_come_from_the_access_token_profile() {
    let harness = Harness::new().await;
    harness
        .server
        .knobs(|knobs| knobs.flaw = Some(Flaw::EmailOnlyInProfile));

    let login = harness.sign_in().await.expect("signed in");

    assert_eq!(login.identity.email, EMAIL);
}

#[tokio::test]
async fn a_dropped_sign_in_frees_the_callback_port() {
    let harness = Harness::new().await;
    // The browser follows nowhere: the page is never answered.
    let auth = PlanAuth::new(PlanAuthDeps {
        endpoints: Endpoints::local(&Url::parse("http://127.0.0.1:9/").expect("a URL")),
        secrets: harness.secrets.clone(),
        browser: Arc::new(IdleBrowser),
        clock: harness.clock.clone(),
    });

    let pending = auth.start().await.expect("the attempt starts");
    assert!(
        std::net::TcpListener::bind(("127.0.0.1", 1455)).is_err(),
        "the attempt listens on the callback port"
    );
    drop(pending);

    assert!(
        std::net::TcpListener::bind(("127.0.0.1", 1455)).is_ok(),
        "dropping the attempt frees the port"
    );
}

/// Opens nothing.
struct IdleBrowser;

impl Browser for IdleBrowser {
    fn open(&self, _url: &Url) -> Result<(), BrowserError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_rotates_the_tokens_together() {
    let harness = Harness::signed_in().await;
    let first = harness.access_token().expect("an access token");
    harness.clock.advance(Duration::from_secs(1800));

    let refreshed = harness.auth.refresh().await.expect("refreshed");

    assert_ne!(refreshed.secret(), first);
    assert_eq!(
        harness.access_token().as_deref(),
        Some(refreshed.secret()),
        "the saved access token is the new one"
    );
    let login = harness.auth.login().expect("reads").expect("a login");
    assert_eq!(
        login.expires_at,
        harness.clock.now() + chrono::Duration::seconds(3600)
    );

    harness.auth.refresh().await.expect("refreshed again");
    let refreshes = harness.server.token_requests("refresh_token");
    assert_eq!(refreshes.len(), 2);
    assert_eq!(refreshes[0]["refresh_token"], "refresh-1");
    assert_eq!(
        refreshes[1]["refresh_token"], "refresh-2",
        "the rotated refresh token replaced the old one"
    );
    for refresh in &refreshes {
        assert_eq!(refresh["client_id"], CODEX_CLIENT_ID);
    }
}

#[tokio::test]
async fn two_concurrent_refreshes_send_one_request() {
    let harness = Harness::signed_in().await;
    harness
        .server
        .knobs(|knobs| knobs.refresh_delay = Some(Duration::from_millis(100)));

    let (first, second) = tokio::join!(harness.auth.refresh(), harness.auth.refresh());

    assert_eq!(first.expect("refreshed"), second.expect("refreshed"));
    assert_eq!(harness.server.token_requests("refresh_token").len(), 1);
}

#[tokio::test]
async fn each_terminal_refresh_error_ends_the_login() {
    for &code in TERMINAL_CODES {
        let harness = Harness::signed_in().await;
        harness
            .server
            .knobs(|knobs| knobs.refresh_refusal = Some((400, code)));

        assert_eq!(
            harness.auth.refresh().await,
            Err(RefreshError::LoginEnded {
                code: code.to_owned()
            })
        );
        assert_eq!(harness.auth.login(), Ok(None), "{code} clears the tokens");
    }
}

#[tokio::test]
async fn a_server_error_on_refresh_keeps_the_tokens() {
    let harness = Harness::signed_in().await;
    let before = harness.access_token();
    harness
        .server
        .knobs(|knobs| knobs.refresh_refusal = Some((503, "temporarily_unavailable")));

    let result = harness.auth.refresh().await;

    assert!(
        matches!(
            result,
            Err(RefreshError::Unavailable(Unreachable::Answer(_)))
        ),
        "got {result:?}"
    );
    assert_eq!(harness.access_token(), before);
}

#[tokio::test]
async fn a_network_error_on_refresh_keeps_the_tokens() {
    let port = CALLBACK_PORT.lock().await;
    callback_port_released().await;
    let closed = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a free port");
    let address = closed.local_addr().expect("the bound address");
    drop(closed);
    let offline = Harness::with_endpoints(
        port,
        Some(Endpoints::local(
            &Url::parse(&format!("http://{address}/")).expect("a URL"),
        )),
    )
    .await;
    // A login saved while the server was reachable.
    let online = PlanAuth::new(PlanAuthDeps {
        endpoints: Endpoints::local(&offline.server.inner.base),
        secrets: offline.secrets.clone(),
        browser: Arc::new(RedirectingBrowser),
        clock: offline.clock.clone(),
    });
    let pending = online.start().await.expect("the attempt starts");
    tokio::time::timeout(Duration::from_secs(10), pending.finish())
        .await
        .expect("the sign-in ends")
        .expect("signed in");
    let before = offline.access_token();

    let result = offline.auth.refresh().await;

    assert!(
        matches!(
            result,
            Err(RefreshError::Unavailable(Unreachable::Network(_)))
        ),
        "got {result:?}"
    );
    assert_eq!(offline.access_token(), before);
}

#[tokio::test]
async fn refreshing_without_a_codex_login_sends_nothing() {
    let harness = Harness::new().await;
    assert_eq!(harness.auth.refresh().await, Err(RefreshError::SignedOut));

    harness
        .secrets
        .write(
            SecretRecord::PlanLogin,
            &json!({
                "subject": SUBJECT,
                "email": EMAIL,
                "id_token": "sharing-id-token",
                "access_token": "sharing-access-token",
                "refresh_token": "sharing-refresh-token",
                "expires_at": "2025-12-31T23:00:00Z",
                "scopes": ["openid", "chatgpt.tokens.use.direct"],
            })
            .to_string(),
        )
        .expect("a plan-sharing login is saved");

    assert_eq!(harness.auth.login(), Ok(None));
    assert_eq!(harness.auth.access_token(), Ok(None));
    assert_eq!(harness.auth.refresh().await, Err(RefreshError::SignedOut));
    assert_eq!(harness.server.token_request_count(), 0);
}

// ---------------------------------------------------------------------------
// Sign-out
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sign_out_clears_the_login_locally_without_a_request() {
    let harness = Harness::signed_in().await;
    let requests = harness.server.token_request_count();

    harness.auth.sign_out().await.expect("signed out");

    assert_eq!(harness.auth.login(), Ok(None));
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
    assert_eq!(harness.server.token_request_count(), requests);
}

#[tokio::test]
async fn sign_out_also_clears_a_plan_sharing_record() {
    let harness = Harness::new().await;
    harness
        .secrets
        .write(SecretRecord::PlanLogin, r#"{"subject":"user-old"}"#)
        .expect("a plan-sharing login is saved");

    harness.auth.sign_out().await.expect("signed out");

    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
}
