//! Sign in with ChatGPT through OpenAI's Codex OAuth client and generation on
//! the Codex backend, through `Core::handle` and the plan provider.
//!
//! One local server plays the authorisation server (`/auth/…`) and the Codex
//! backend (`/api/…`) on `127.0.0.1`; the browser follows the authorisation
//! page's redirect to the core's callback on the registered Codex loopback
//! address. The secret store is in memory and the clock is fake.
//!
//! The Codex callback port is fixed, so the tests that sign in take
//! [`CALLBACK_PORT`] first and run one at a time.

mod support;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::ports::{Browser, BrowserError, Clock, Endpoints, SecretRecord, SecretStore};
use atlas_core::protocol::{
    CoreError, CoreEvent, CoreRequest, CoreResponse, Model, ModelList, PlanUsage, Session,
};
use atlas_core::provider::chatgpt::{ChatGptProvider, models_url, responses_url};
use atlas_core::provider::{ProviderChoice, ProviderRequest, Task, complete_text, for_session};
use atlas_core::store::Store;
use atlas_core::testing::{
    FakeClock, FakePdfExporter, FakeTokenSource, MemorySecretStore, RecordingEvents,
};
use atlas_core::{Core, CoreDeps};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Json, Redirect, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::{Jwk, JwkSet, PublicKeyUse};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::server::{RecordedRequest, Reply, Route, TestServer};
use tempfile::TempDir;
use tokio::net::TcpListener;
use url::Url;

/// OpenAI's Codex OAuth client, the one OMP signs in with.
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// The only callback OpenAI allowlists for that client.
const CODEX_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const CODEX_PORT: u16 = 1455;
const CODEX_SCOPES: &str =
    "openid profile email offline_access api.connectors.read api.connectors.invoke";
/// Atlas names itself, rather than posing as OMP or the Codex CLI.
const ORIGINATOR: &str = "atlas";
/// The scope of ChatGPT plan sharing, which Codex sign-in never asks for.
const SHARING_SCOPE: &str = "chatgpt.tokens.use.direct";

const AUTHORIZE: &str = "/auth/oauth/authorize";
const TOKEN: &str = "/auth/oauth/token";
const DISCOVERY: &str = "/auth/.well-known/openid-configuration";
const JWKS: &str = "/auth/.well-known/jwks.json";
const MODELS: &str = "/api/models";
const RESPONSES: &str = "/api/responses";

const CODE: &str = "codex-code-5Rk2";
const SUBJECT: &str = "user-Codex7";
const EMAIL: &str = "lin@example.com";
const ACCOUNT_ID: &str = "acct-enterprise-42";
const KEY_ID: &str = "codex-key-1";
/// The claim namespace of OpenAI's tokens that carries the ChatGPT account.
const AUTH_CLAIM: &str = "https://api.openai.com/auth";

/// The fake server's signing key. A test key, used nowhere else.
const SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvwIBADANBgkqhkiG9w0BAQEFAASCBKkwggSlAgEAAoIBAQDHRdJQOn0qc91I
lhfJmh1ctcsVf4fr7Xa599xGDbr1SICaretP9GjcwFQsGeuHt3+ULuOyCDxLuLhE
ZsFK7I+QYHFrDGLJGO2JqTx2TAioQc2C0Wkqs4Vfr35U0H1T5Kr5Q5sZJNEJhCnK
5lU73CMPx1MMoyVDtYcyP9sAC+p/kenxR2kSAcSKgFLEUBvVbQWunDdws3Cb0X6e
Hr7cKPzo8h0WJ8xDgoWjQFlx3RJCgzQCLBGnLztfq92VYJwO9Ffu3I7FUWAFpbNr
F2xm59G8RlH/P7MDQ/iomF576X4FumuMn377wFFrSD5QCeIQyBBuSNo5QtE9Hfoq
3X6Fq50ZAgMBAAECggEAAi4cX+5T4jwERsAFoJoECjie8SxFA2rEqsIwcb/CtN5H
MeoHMge9gfTEtvrNnd1/EtL0gMUgS7/InE73fGdcJrs3YnFvPz37k077r1ujmJun
sO+zk9kE84gK/MiPJnTQ/gskTkEd3FnKz0J32kDNTWXj3baHONdqlD0omzaGKUEd
ed9+YjehFkaCcvq069XTeENMDyJEaJ7sywxgrqwfQfnZIpIIre81g/Eh/B38Ma3k
4cLPfIeqZ92m6wnKAWRN53Z9X/wfNCSlxzbW+7E4WqK/RbxKNxQQIHv1W0qzNqcx
0LhnZIgYAL0GATBBQAIQdn6hClCaMIxmRPmJXlej1QKBgQDj56mXNYj4FcgsZB9C
0oxkr1DdqjQ6XpJsgr7OQjeA/powUqMY3jvUZaA2mzwByKMz78FCetPEJd3nlX6j
sBXWBMePAx1UmSipQgKU+WVPf1aIoa603k18GnwMZhF5RITR8ydr185MPQtl62rz
PEFgvUJBz3yyqeVekSMSAnYoSwKBgQDf1pHyGSoj9y8kxeR52B9cDl+WUaCUieQ7
4/jZu5M/5aoBUqHjm5YnAtbKM9QCq9Rf4fgcLE2QYtgJCI5oD3621wSiCzhDByQp
/JZgsw+GN4t9f/+xmwt6kxQqXTt0Bq/mCYCae5q1+hmW+nyADYpyFJumADd6o29M
6timZZI5qwKBgQDf39/OM76axPrf8HJs99sFxNoRBXNMqz327fB1OMLBKllYgrTI
ZvYqzLG3MoKd1fPLvwb/QRzpH7F+HnJKZVzt4cAOkiarxdAVhEx4/wwwLYxZp0mx
TnabJIEaxOEBWPeILe1ZmKKEbarWsY1b5qLqXr/JWgLKlu5EGRp0RIrBRQKBgQCB
NNSOESSYGikmauzVwIFpnAplNL0iwO3z/EY7OOFosuONW+uC/N6RqjveAEjTOeif
aErVyGUFgWj+3eeF/dvBIyDTgPJ7DnY7nkgXjYTajtmAGhC4rqzTlm6/fY3CDdIf
UUDg65SEkteOpQyUxjwyWd721Y0wx4KJvc5NTrg3fQKBgQDAdNQs5omvBCTjgT1W
kcqbBXKyPukHFGzd2ok1hOoUURfuKObCsMafwU0eHogENbJqr1cR2ZpfQeKna1KG
rjKosyLx8bP1jedh35CouWEtyPphxf8hyI3yu9maUbcptYI750FMuDCLc8VZqxJv
vlmXEqvKDaJbDuudP4oRvJ1NXw==
-----END PRIVATE KEY-----
";

/// Held by every test that binds or needs the fixed callback port.
static CALLBACK_PORT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

fn b64(bytes: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// A JWT-shaped access token: the client reads its claims and never checks
/// its signature.
fn access_token(claims: &Value) -> String {
    format!(
        "{}.{}.{}",
        b64(r#"{"alg":"RS256","typ":"JWT"}"#),
        b64(claims.to_string()),
        b64("not-checked-by-the-client")
    )
}

/// An access token for `ACCOUNT_ID` whose auth claim also holds `auth`.
fn account_token(auth: Value) -> String {
    let mut claim = json!({
        "chatgpt_account_id": ACCOUNT_ID,
        "chatgpt_plan_type": "enterprise",
    });
    if let (Some(claim), Value::Object(extra)) = (claim.as_object_mut(), auth) {
        claim.extend(extra);
    }
    access_token(&json!({ "sub": SUBJECT, AUTH_CLAIM: claim }))
}

fn code_challenge(verifier: &str) -> String {
    b64(Sha256::digest(verifier.as_bytes()))
}

fn is_version(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

// ---------------------------------------------------------------------------
// The authorisation server and the Codex backend
// ---------------------------------------------------------------------------

/// A request the server received, its query and form decoded.
#[derive(Debug, Clone)]
struct Seen {
    method: Method,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    form: HashMap<String, String>,
}

#[derive(Default)]
struct Knobs {
    /// The callback carries a state other than the one sent.
    wrong_state: bool,
    /// The token endpoint refuses the code.
    refuse_code: bool,
}

#[derive(Default)]
struct ServerState {
    seen: Vec<Seen>,
    knobs: Knobs,
    /// The access tokens issued, in order.
    issued: Vec<String>,
    /// The nonce of the last authorisation, when one was sent.
    nonce: Option<String>,
}

struct ServerInner {
    base: Url,
    clock: Arc<FakeClock>,
    state: Mutex<ServerState>,
}

impl ServerInner {
    fn state(&self) -> std::sync::MutexGuard<'_, ServerState> {
        self.state.lock().expect("server lock poisoned")
    }

    fn issuer(&self) -> String {
        format!("{}auth", self.base)
    }

    fn id_token(&self, nonce: Option<&str>) -> String {
        let now = self.clock.now().timestamp();
        let mut claims = json!({
            "iss": self.issuer(),
            "sub": SUBJECT,
            "email": EMAIL,
            "aud": CODEX_CLIENT_ID,
            "iat": now,
            "exp": now + 3600,
            AUTH_CLAIM: { "chatgpt_account_id": ACCOUNT_ID, "chatgpt_plan_type": "enterprise" },
        });
        if let Some(nonce) = nonce {
            claims["nonce"] = json!(nonce);
        }
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KEY_ID.to_owned());
        let key = EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).expect("a valid test key");
        jsonwebtoken::encode(&header, &claims, &key).expect("a signed ID token")
    }

    /// Issues the next tokens: `refresh-<n>` with an access token for the
    /// account.
    fn issue(&self) -> Value {
        let mut state = self.state();
        let n = state.issued.len() + 1;
        let now = self.clock.now().timestamp();
        let token = access_token(&json!({
            "sub": SUBJECT,
            "iat": now,
            "exp": now + 3600,
            "issued": n,
            AUTH_CLAIM: { "chatgpt_account_id": ACCOUNT_ID, "chatgpt_plan_type": "enterprise" },
            "https://api.openai.com/profile": { "email": EMAIL },
        }));
        state.issued.push(token.clone());
        let nonce = state.nonce.clone();
        drop(state);
        json!({
            "access_token": token,
            "refresh_token": format!("refresh-{n}"),
            "id_token": self.id_token(nonce.as_deref()),
            "expires_in": 3600,
            "token_type": "Bearer",
            "scope": "openid profile email offline_access",
        })
    }
}

fn decode(pairs: &str) -> HashMap<String, String> {
    url::form_urlencoded::parse(pairs.as_bytes())
        .into_owned()
        .collect()
}

fn oauth_error(error: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": error, "error_description": "refused by the test server" })),
    )
        .into_response()
}

/// The Codex model list: two models to show and one hidden.
fn codex_models() -> Value {
    json!({
        "models": [
            {
                "slug": "gpt-5.5",
                "display_name": "GPT-5.5",
                "visibility": "list",
                "priority": 0,
                "supported_reasoning_levels": [{ "effort": "medium" }, { "effort": "high" }],
            },
            {
                "slug": "gpt-5.4-mini",
                "display_name": "GPT-5.4 Mini",
                "visibility": "list",
                "priority": 4,
            },
            {
                "slug": "codex-auto-review",
                "display_name": "Codex Auto Review",
                "visibility": "hide",
                "priority": 90,
            },
        ]
    })
}

fn listed_models() -> ModelList {
    let model = |slug: &str, display_name: &str| Model {
        slug: slug.to_owned(),
        display_name: display_name.to_owned(),
    };
    ModelList {
        models: vec![
            model("gpt-5.5", "GPT-5.5"),
            model("gpt-5.4-mini", "GPT-5.4 Mini"),
        ],
        current: Some("gpt-5.5".to_owned()),
    }
}

async fn answer(
    State(server): State<Arc<ServerInner>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let seen = Seen {
        method,
        path: uri.path().to_owned(),
        query: decode(uri.query().unwrap_or_default()),
        headers: headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
        form: decode(&String::from_utf8_lossy(&body)),
    };
    server.state().seen.push(seen.clone());

    match (seen.method.as_str(), seen.path.as_str()) {
        ("GET", AUTHORIZE) => {
            let (Some(redirect), Some(state)) =
                (seen.query.get("redirect_uri"), seen.query.get("state"))
            else {
                return oauth_error("invalid_request");
            };
            let Ok(mut callback) = Url::parse(redirect) else {
                return oauth_error("invalid_request");
            };
            let wrong_state = {
                let mut server_state = server.state();
                server_state.nonce = seen.query.get("nonce").cloned();
                server_state.knobs.wrong_state
            };
            callback
                .query_pairs_mut()
                .append_pair("code", CODE)
                .append_pair(
                    "state",
                    if wrong_state {
                        "not-the-state"
                    } else {
                        state.as_str()
                    },
                );
            Redirect::to(callback.as_str()).into_response()
        }
        ("POST", TOKEN) => match seen.form.get("grant_type").map(String::as_str) {
            Some("authorization_code") => {
                let refuse = server.state().knobs.refuse_code;
                if refuse || seen.form.get("code").map(String::as_str) != Some(CODE) {
                    return oauth_error("invalid_grant");
                }
                Json(server.issue()).into_response()
            }
            Some("refresh_token") => Json(server.issue()).into_response(),
            _ => oauth_error("unsupported_grant_type"),
        },
        ("GET", DISCOVERY) => {
            let base = server.base.as_str().trim_end_matches('/');
            Json(json!({
                "issuer": server.issuer(),
                "authorization_endpoint": format!("{base}{AUTHORIZE}"),
                "token_endpoint": format!("{base}{TOKEN}"),
                "jwks_uri": format!("{base}{JWKS}"),
            }))
            .into_response()
        }
        ("GET", JWKS) => {
            let key = EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).expect("a valid test key");
            let mut jwk = Jwk::from_encoding_key(&key, Algorithm::RS256).expect("a public JWK");
            jwk.common.key_id = Some(KEY_ID.to_owned());
            jwk.common.public_key_use = Some(PublicKeyUse::Signature);
            Json(serde_json::to_value(JwkSet { keys: vec![jwk] }).expect("a JWK set"))
                .into_response()
        }
        ("GET", MODELS) => Json(codex_models()).into_response(),
        _ => (StatusCode::NOT_FOUND, Body::from("no such route")).into_response(),
    }
}

struct CodexServer {
    inner: Arc<ServerInner>,
}

impl CodexServer {
    async fn start(clock: Arc<FakeClock>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let inner = Arc::new(ServerInner {
            base: Url::parse(&format!("http://{address}/")).expect("a valid URL"),
            clock,
            state: Mutex::default(),
        });
        let app = Router::new().fallback(answer).with_state(inner.clone());
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("the test server runs");
        });
        Self { inner }
    }

    fn endpoints(&self) -> Endpoints {
        Endpoints::local(&self.inner.base)
    }

    fn set(&self, change: impl FnOnce(&mut Knobs)) {
        change(&mut self.inner.state().knobs);
    }

    fn seen(&self) -> Vec<Seen> {
        self.inner.state().seen.clone()
    }

    fn seen_at(&self, method: Method, path: &str) -> Vec<Seen> {
        self.seen()
            .into_iter()
            .filter(|seen| seen.method == method && seen.path == path)
            .collect()
    }

    fn token_requests(&self, grant_type: &str) -> Vec<HashMap<String, String>> {
        self.seen_at(Method::POST, TOKEN)
            .into_iter()
            .map(|seen| seen.form)
            .filter(|form| form.get("grant_type").map(String::as_str) == Some(grant_type))
            .collect()
    }

    fn issued(&self) -> Vec<String> {
        self.inner.state().issued.clone()
    }
}

// ---------------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------------

/// Records each page it opens; when `follow` is set, visits it and follows
/// its redirects as the user's browser would after the user approves.
struct LoopbackBrowser {
    follow: bool,
    opened: Mutex<Vec<Url>>,
}

impl LoopbackBrowser {
    fn opened(&self) -> Vec<Url> {
        self.opened.lock().expect("browser lock poisoned").clone()
    }
}

impl Browser for LoopbackBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        self.opened
            .lock()
            .expect("browser lock poisoned")
            .push(url.clone());
        if self.follow {
            let url = url.clone();
            tokio::spawn(async move {
                let _ = reqwest::Client::new().get(url).send().await;
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

/// One installation: its data folder, secret store and clock, from which a
/// core is built for each launch.
struct Installation {
    server: CodexServer,
    clock: Arc<FakeClock>,
    secrets: Arc<MemorySecretStore>,
    browser: Arc<LoopbackBrowser>,
    events: Arc<RecordingEvents>,
    dir: TempDir,
}

impl Installation {
    async fn new(follow: bool) -> Self {
        let clock = Arc::new(FakeClock::default());
        Self {
            server: CodexServer::start(clock.clone()).await,
            clock,
            secrets: Arc::new(MemorySecretStore::default()),
            browser: Arc::new(LoopbackBrowser {
                follow,
                opened: Mutex::default(),
            }),
            events: Arc::new(RecordingEvents::default()),
            dir: tempfile::tempdir().expect("a temp dir"),
        }
    }

    fn database(&self) -> std::path::PathBuf {
        self.dir.path().join("atlas.sqlite3")
    }

    /// The core as the app builds it: the plan provider on the server's
    /// endpoints.
    fn launch(&self) -> Core {
        let endpoints = self.server.endpoints();
        Core::new(CoreDeps {
            database_path: self.database(),
            provider: for_session(ProviderChoice::Plan, &endpoints, self.clock.clone()),
            events: self.events.clone(),
            clock: self.clock.clone(),
            secrets: self.secrets.clone(),
            browser: self.browser.clone(),
            pdf: Arc::new(FakePdfExporter::default()),
            endpoints,
        })
        .expect("the core starts")
    }

    /// Signs in through the browser and returns the session once it settles.
    async fn sign_in(&self, core: &Core) -> Session {
        assert_eq!(
            core.handle(CoreRequest::SignIn).await,
            Ok(CoreResponse::Session {
                session: Session::SigningIn,
            })
        );
        let page = self.browser.opened().pop().expect("the browser was opened");
        assert_eq!(page.path(), AUTHORIZE, "the Codex authorisation page");
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match session(core).await {
                    Session::SigningIn => tokio::time::sleep(Duration::from_millis(20)).await,
                    session => return session,
                }
            }
        })
        .await
        .expect("the sign-in settles")
    }

    /// The last model list reported, waiting for one to be.
    async fn reported_models(&self, seen: &mut Vec<CoreEvent>) -> ModelList {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                seen.extend(self.events.take());
                if let Some(list) = seen.iter().rev().find_map(|event| match event {
                    CoreEvent::ModelsChanged { list } => Some(list.clone()),
                    _ => None,
                }) {
                    return list;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("a model list is reported")
    }
}

async fn session(core: &Core) -> Session {
    match core.handle(CoreRequest::GetSession).await {
        Ok(CoreResponse::Session { session }) => session,
        other => panic!("expected a session, got {other:?}"),
    }
}

fn signed_in() -> Session {
    Session::SignedIn {
        account: atlas_core::protocol::Account {
            email: EMAIL.to_owned(),
        },
        plan_usage: PlanUsage::On,
    }
}

/// Waits until nothing listens on the callback port any more.
async fn callback_port_is_free() -> bool {
    for _ in 0..100 {
        if std::net::TcpListener::bind(("127.0.0.1", CODEX_PORT)).is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

// ---------------------------------------------------------------------------
// The authorisation page and the callback port
// ---------------------------------------------------------------------------

#[tokio::test]
async fn signing_in_opens_the_codex_authorization_page_and_cancelling_frees_the_callback_port() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(false).await;
    let core = installation.launch();

    let mut attempts = Vec::new();
    for _ in 0..2 {
        assert_eq!(
            core.handle(CoreRequest::SignIn).await,
            Ok(CoreResponse::Session {
                session: Session::SigningIn,
            })
        );
        let page = installation
            .browser
            .opened()
            .pop()
            .expect("the browser was opened");
        assert_eq!(page.path(), AUTHORIZE);
        assert_eq!(page.host_str(), Some("127.0.0.1"));
        let query: HashMap<String, String> = page.query_pairs().into_owned().collect();
        assert_eq!(query["client_id"], CODEX_CLIENT_ID);
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["redirect_uri"], CODEX_REDIRECT_URI);
        assert_eq!(query["scope"], CODEX_SCOPES);
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["id_token_add_organizations"], "true");
        assert_eq!(query["codex_cli_simplified_flow"], "true");
        assert_eq!(query["originator"], ORIGINATOR);
        assert_eq!(query["code_challenge"].len(), 43, "an S256 challenge");
        assert!(query["state"].len() >= 16, "a random state");
        for registration in [
            "agent_name_hint",
            "ext_agent_host_id",
            "resource",
            "id_token_hint",
            "prompt",
        ] {
            assert!(
                !query.contains_key(registration),
                "{registration} belongs to plan sharing, found {query:?}"
            );
        }
        attempts.push((query["state"].clone(), query["code_challenge"].clone()));

        let cancelled = core.handle(CoreRequest::CancelSignIn).await;
        assert!(
            matches!(
                cancelled,
                Ok(CoreResponse::Session {
                    session: Session::SignedOut { .. },
                })
            ),
            "cancelled, got {cancelled:?}"
        );
        assert!(
            callback_port_is_free().await,
            "a cancelled sign-in releases port {CODEX_PORT}"
        );
    }

    assert_ne!(
        attempts[0].0, attempts[1].0,
        "each attempt has its own state"
    );
    assert_ne!(
        attempts[0].1, attempts[1].1,
        "each attempt has its own PKCE verifier"
    );
    assert!(
        installation.server.seen().is_empty(),
        "starting a sign-in registers nothing: {:?}",
        installation.server.seen()
    );
}

#[tokio::test]
async fn a_busy_callback_port_fails_the_sign_in_clearly_without_opening_the_browser() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(true).await;
    let core = installation.launch();
    let _occupied = std::net::TcpListener::bind(("127.0.0.1", CODEX_PORT))
        .expect("port 1455 is free for this test");

    let started = core.handle(CoreRequest::SignIn).await;

    let error = started.expect_err("the sign-in cannot listen for its callback");
    assert!(
        format!("{error:?}").contains("1455"),
        "the error names the busy port, got {error:?}"
    );
    assert!(installation.browser.opened().is_empty());
    assert!(matches!(session(&core).await, Session::SignedOut { .. }));
}

// ---------------------------------------------------------------------------
// The code exchange
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_loopback_callback_completes_a_codex_sign_in_that_enables_generation() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(true).await;
    let core = installation.launch();

    assert_eq!(installation.sign_in(&core).await, signed_in());

    let page = installation.browser.opened().pop().expect("a page");
    let query: HashMap<String, String> = page.query_pairs().into_owned().collect();
    let exchanges = installation.server.token_requests("authorization_code");
    assert_eq!(exchanges.len(), 1);
    let exchange = &exchanges[0];
    assert_eq!(exchange["client_id"], CODEX_CLIENT_ID);
    assert_eq!(exchange["code"], CODE);
    assert_eq!(exchange["redirect_uri"], CODEX_REDIRECT_URI);
    assert_eq!(
        code_challenge(&exchange["code_verifier"]),
        query["code_challenge"],
        "the verifier matches the S256 challenge"
    );
    assert!(!exchange.contains_key("resource"), "found {exchange:?}");
}

#[tokio::test]
async fn after_sign_in_the_model_list_comes_from_codex_discovery() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(true).await;
    let core = installation.launch();
    assert_eq!(installation.sign_in(&core).await, signed_in());

    let mut events = Vec::new();
    assert_eq!(
        installation.reported_models(&mut events).await,
        listed_models()
    );

    let listings = installation.server.seen_at(Method::GET, MODELS);
    assert_eq!(listings.len(), 1, "the list was read once");
    let listing = &listings[0];
    let version = listing
        .query
        .get("client_version")
        .expect("the listing names the client version");
    assert!(is_version(version), "a client version, got {version}");
    assert_eq!(listing.headers.get("version"), Some(version));
    assert_eq!(
        listing.headers.get("authorization"),
        Some(&format!("Bearer {}", installation.server.issued()[0]))
    );
    assert_eq!(
        listing
            .headers
            .get("chatgpt-account-id")
            .map(String::as_str),
        Some(ACCOUNT_ID)
    );
    assert_eq!(
        listing.headers.get("originator").map(String::as_str),
        Some(ORIGINATOR)
    );
}

#[tokio::test]
async fn a_callback_with_another_state_saves_no_login_and_exchanges_nothing() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(true).await;
    installation.server.set(|knobs| knobs.wrong_state = true);
    let core = installation.launch();

    let settled = installation.sign_in(&core).await;

    assert!(
        matches!(settled, Session::SignedOut { .. }),
        "got {settled:?}"
    );
    assert!(installation.server.seen_at(Method::POST, TOKEN).is_empty());
    assert_eq!(
        core.handle(CoreRequest::ListModels).await,
        Err(CoreError::SignInRequired)
    );
}

#[tokio::test]
async fn a_refused_code_saves_no_login_and_reports_no_secret() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(true).await;
    installation.server.set(|knobs| knobs.refuse_code = true);
    let core = installation.launch();

    let settled = installation.sign_in(&core).await;

    assert!(
        matches!(settled, Session::SignedOut { .. }),
        "got {settled:?}"
    );
    let exchanges = installation.server.token_requests("authorization_code");
    assert_eq!(exchanges.len(), 1);
    let verifier = &exchanges[0]["code_verifier"];
    let reported = format!(
        "{}{}",
        serde_json::to_string(&settled).expect("a session serialises"),
        serde_json::to_string(&installation.events.take()).expect("events serialise")
    );
    for secret in [CODE, verifier.as_str()] {
        assert!(
            !reported.contains(secret),
            "a secret reached the interface: {reported}"
        );
    }
    assert_eq!(
        core.handle(CoreRequest::ListModels).await,
        Err(CoreError::SignInRequired)
    );
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_expired_codex_login_refreshes_with_the_codex_client_and_rotates_its_refresh_token() {
    let _port = CALLBACK_PORT.lock().await;
    let installation = Installation::new(true).await;
    let first = installation.launch();
    assert_eq!(installation.sign_in(&first).await, signed_in());
    installation.reported_models(&mut Vec::new()).await;
    drop(first);

    for (launch, spent) in [(1, "refresh-1"), (2, "refresh-2")] {
        installation.clock.advance(Duration::from_secs(2 * 3600));
        let core = installation.launch();

        let listed = core.handle(CoreRequest::ListModels).await;
        assert!(
            matches!(listed, Ok(CoreResponse::Models { .. })),
            "launch {launch} lists the models, got {listed:?}"
        );

        let refreshes = installation.server.token_requests("refresh_token");
        assert_eq!(refreshes.len(), launch, "one refresh per expired launch");
        let refresh = &refreshes[launch - 1];
        assert_eq!(refresh["client_id"], CODEX_CLIENT_ID);
        assert_eq!(
            refresh["refresh_token"], spent,
            "the rotated token is spent"
        );
        assert!(!refresh.contains_key("resource"), "found {refresh:?}");
        let listing = installation
            .server
            .seen_at(Method::GET, MODELS)
            .pop()
            .expect("a listing");
        assert_eq!(
            listing.headers.get("authorization"),
            Some(&format!("Bearer {}", installation.server.issued()[launch])),
            "only the refreshed token is presented"
        );
        assert_eq!(session(&core).await, signed_in());
    }
}

// ---------------------------------------------------------------------------
// A login saved by ChatGPT plan sharing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_installation_with_a_plan_sharing_login_needs_sign_in_and_keeps_its_courses() {
    let installation = Installation::new(false).await;
    let course = Store::open(&installation.database(), installation.clock.clone())
        .expect("the store opens")
        .create_course("Fluid dynamics")
        .expect("a course is stored");
    // The login record plan sharing saved: an expired token for the same
    // account, granted the sharing scope.
    let sharing_token = account_token(json!({}));
    installation
        .secrets
        .write(
            SecretRecord::PlanLogin,
            &json!({
                "subject": SUBJECT,
                "email": EMAIL,
                "id_token": sharing_token,
                "access_token": sharing_token,
                "refresh_token": "sharing-refresh-token",
                "expires_at": "2025-12-31T23:00:00Z",
                "scopes": [
                    "openid", "profile", "email", "offline_access", "resource.invoke",
                    SHARING_SCOPE,
                ],
                "plan_not_allowed": false,
            })
            .to_string(),
        )
        .expect("the old login is saved");

    let core = installation.launch();

    let current = session(&core).await;
    assert!(
        matches!(current, Session::SignedOut { .. }),
        "the sharing login is not a Codex login, got {current:?}"
    );
    assert_eq!(
        core.handle(CoreRequest::ListModels).await,
        Err(CoreError::SignInRequired)
    );
    match core.handle(CoreRequest::ListCourses).await {
        Ok(CoreResponse::Courses { courses }) => {
            assert_eq!(
                courses.iter().map(|summary| summary.id).collect::<Vec<_>>(),
                [course.id]
            );
        }
        other => panic!("expected the courses, got {other:?}"),
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        installation.server.seen().is_empty(),
        "the sharing tokens are neither refreshed nor sent: {:?}",
        installation.server.seen()
    );
}

// ---------------------------------------------------------------------------
// The Codex backend
// ---------------------------------------------------------------------------

#[test]
fn the_production_generation_route_is_the_codex_backend() {
    let api = Endpoints::production().api;

    assert_eq!(
        responses_url(&api).as_str(),
        "https://chatgpt.com/backend-api/codex/responses"
    );
    let models = models_url(&api);
    assert_eq!(models.host_str(), Some("chatgpt.com"));
    assert_eq!(models.path(), "/backend-api/codex/models");
}

/// A whole streamed response whose text is `text`, in one delta.
fn streamed(text: &str) -> String {
    let delta = json!({
        "type": "response.output_text.delta",
        "sequence_number": 0,
        "item_id": "msg_codex",
        "output_index": 0,
        "content_index": 0,
        "delta": text,
    });
    let completed = json!({
        "type": "response.completed",
        "sequence_number": 1,
        "response": { "id": "resp_codex", "status": "completed", "output": [] },
    });
    format!(
        "event: response.output_text.delta\ndata: {delta}\n\n\
         event: response.completed\ndata: {completed}\n\n"
    )
}

/// Streams one request on `model` with `token` and returns what the backend
/// received.
async fn generate(server: &TestServer, token: &str, model: &str) -> RecordedRequest {
    let before = server.requests().len();
    let provider = ChatGptProvider::new(
        &server.endpoints(),
        Arc::new(FakeTokenSource::new(token)),
        Arc::new(FakeClock::default()),
    );
    let mut request = ProviderRequest::new(Task::WriteSection, "Write the section.", "Context.");
    request.model = Some(model.to_owned());

    let text = complete_text(&provider, request, |_| {})
        .await
        .expect("the response completes");

    assert_eq!(text, "Hello from Codex");
    let requests = server.requests();
    assert_eq!(requests.len(), before + 1, "one request: {requests:?}");
    requests[before].clone()
}

#[tokio::test]
async fn a_codex_responses_request_carries_the_account_routing_and_client_headers() {
    let server = TestServer::start([Route::post(
        RESPONSES,
        [Reply::sse(&streamed("Hello from Codex"))],
    )])
    .await;
    let token = account_token(json!({}));

    let request = generate(&server, &token, "gpt-5.5").await;

    assert_eq!(request.method, "POST");
    assert_eq!(request.path, RESPONSES);
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {token}").as_str())
    );
    assert_eq!(request.header("chatgpt-account-id"), Some(ACCOUNT_ID));
    assert_eq!(request.header("originator"), Some(ORIGINATOR));
    let version = request.header("version").expect("a client version");
    assert!(is_version(version), "a client version, got {version}");
    let agent = request.header("user-agent").expect("a user agent");
    assert!(
        agent.starts_with("atlas/"),
        "Atlas's own agent, got {agent}"
    );
    assert_eq!(
        request.header("x-codex-routing-hint"),
        Some("model=gpt-5.5")
    );
    assert_eq!(
        request.header("openai-beta"),
        None,
        "SSE carries no beta header"
    );
    assert_eq!(
        request.header("x-openai-internal-codex-residency"),
        None,
        "an account without residency declares none"
    );
    let body: Value = serde_json::from_slice(&request.body).expect("a JSON body");
    assert_eq!(body["model"], "gpt-5.5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
}

#[tokio::test]
async fn a_region_pinned_workspace_declares_its_residency_from_the_token() {
    let server = TestServer::start([Route::post(
        RESPONSES,
        [Reply::sse(&streamed("Hello from Codex"))],
    )])
    .await;
    let cases = [
        (json!({ "chatgpt_data_residency": "eu" }), Some("eu")),
        (
            json!({ "chatgpt_data_residency": "NO_CONSTRAINT", "chatgpt_compute_residency": "us" }),
            None,
        ),
        (json!({ "chatgpt_compute_residency": "eu" }), Some("eu")),
        (
            json!({ "chatgpt_data_residency": "  ", "chatgpt_compute_residency": "us" }),
            Some("us"),
        ),
    ];

    for (claims, expected) in cases {
        let request = generate(&server, &account_token(claims.clone()), "gpt-5.5").await;
        assert_eq!(
            request.header("x-openai-internal-codex-residency"),
            expected,
            "for the claims {claims}"
        );
    }
}
