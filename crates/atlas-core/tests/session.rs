//! The session and external links through `Core::handle`, against a fake
//! authorisation server on `127.0.0.1` with a test signing key, a secret store
//! in memory, a browser that follows only local redirects, and a fake clock.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use atlas_core::ports::{
    Browser, BrowserError, Clock, Endpoints, EventSink, SecretError, SecretRecord, SecretStore,
};
use atlas_core::protocol::{
    Account, CoreError, CoreEvent, CoreRequest, CoreResponse, Model, PlanUsage, Session,
    SignedOutReason, UsageState,
};
use atlas_core::provider::{
    FakeProvider, FakeScript, ModelProvider, ProviderError, ProviderRequest, ProviderStream,
    TokenSource,
};
use atlas_core::testing::{FakeClock, FakePdfExporter, MemorySecretStore};
use atlas_core::{Core, CoreDeps};
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use jsonwebtoken::jwk::{Jwk, JwkSet, PublicKeyUse};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;
use url::Url;

const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const SUBJECT: &str = "user-7Qe2";
const EMAIL: &str = "ada@example.com";
const KEY_ID: &str = "test-key-1";

const AUTHORIZE: &str = "/auth/api/accounts/authorize";
const TOKEN: &str = "/auth/api/accounts/oauth/token";
const REVOKE: &str = "/auth/oauth/revoke";

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

// ---------------------------------------------------------------------------
// The fake authorisation server
// ---------------------------------------------------------------------------

/// How the server behaves; a test changes it between steps.
#[derive(Debug, Clone)]
struct Knobs {
    /// Whether the user grants `chatgpt.tokens.use.direct`.
    grant_plan_scope: bool,
    /// The user declines on the consent page.
    decline: bool,
    /// Every refresh token is refused as `invalid_grant`: the login was
    /// revoked.
    revoked: bool,
}

impl Default for Knobs {
    fn default() -> Self {
        Self {
            grant_plan_scope: true,
            decline: false,
            revoked: false,
        }
    }
}

#[derive(Debug, Clone)]
struct Seen {
    path: String,
    params: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct Grant {
    client_id: String,
    nonce: String,
    scope: String,
}

#[derive(Default)]
struct AuthState {
    knobs: Knobs,
    seen: Vec<Seen>,
    registered: Vec<String>,
    codes: HashMap<String, Grant>,
    /// Refresh tokens still valid, with their client id and scope.
    live_refresh: HashMap<String, (String, String)>,
    /// Every access token issued, in order.
    access_tokens: Vec<String>,
    /// Every token issued: access, refresh and ID tokens.
    issued: Vec<String>,
    counter: u32,
}

struct AuthServerInner {
    base: Url,
    clock: Arc<FakeClock>,
    state: Mutex<AuthState>,
}

impl AuthServerInner {
    fn issuer(&self) -> String {
        format!("{}auth", self.base)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, AuthState> {
        self.state.lock().expect("server lock poisoned")
    }

    fn record(&self, path: &str, params: &HashMap<String, String>) {
        self.state().seen.push(Seen {
            path: path.to_owned(),
            params: params.clone(),
        });
    }

    fn id_token(&self, grant: &Grant) -> String {
        let now = self.clock.now().timestamp();
        let claims = json!({
            "iss": self.issuer(),
            "sub": SUBJECT,
            "email": EMAIL,
            "aud": grant.client_id,
            "nonce": grant.nonce,
            "iat": now,
            "exp": now + 3600,
        });
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KEY_ID.to_owned());
        let key = EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).expect("a valid test key");
        jsonwebtoken::encode(&header, &claims, &key).expect("a signed ID token")
    }
}

fn oauth_error(status: u16, body: Value) -> Response {
    (
        StatusCode::from_u16(status).expect("a valid status"),
        Json(body),
    )
        .into_response()
}

async fn discovery(State(server): State<Arc<AuthServerInner>>) -> Json<Value> {
    let base = server.base.as_str().trim_end_matches('/');
    Json(json!({
        "issuer": server.issuer(),
        "authorization_endpoint": format!("{base}{AUTHORIZE}"),
        "token_endpoint": format!("{base}{TOKEN}"),
        "jwks_uri": format!("{base}/auth/.well-known/jwks.json"),
        "revocation_endpoint": format!("{base}{REVOKE}"),
    }))
}

async fn jwks() -> Json<Value> {
    let key = EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).expect("a valid test key");
    let mut jwk = Jwk::from_encoding_key(&key, Algorithm::RS256).expect("a public JWK");
    jwk.common.key_id = Some(KEY_ID.to_owned());
    jwk.common.public_key_use = Some(PublicKeyUse::Signature);
    Json(serde_json::to_value(JwkSet { keys: vec![jwk] }).expect("a JWK set"))
}

async fn authorize(
    State(server): State<Arc<AuthServerInner>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    server.record(AUTHORIZE, &params);
    let param = |name: &str| params.get(name).cloned().unwrap_or_default();
    let mut callback = Url::parse(&param("redirect_uri")).expect("a redirect URI");
    let mut state = server.state();
    let knobs = state.knobs.clone();
    if knobs.decline {
        callback
            .query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &param("state"));
        return Redirect::to(callback.as_str()).into_response();
    }
    let requested = param("client_id");
    let (client_id, named) = if requested == "dynamic_agent_client" {
        state.counter += 1;
        let issued = format!("oaiapp_atlas_{}", state.counter);
        state.registered.push(issued.clone());
        (issued.clone(), Some(issued))
    } else if state.registered.contains(&requested) {
        (requested, None)
    } else {
        callback
            .query_pairs_mut()
            .append_pair("error", "unauthorized_client")
            .append_pair("state", &param("state"));
        return Redirect::to(callback.as_str()).into_response();
    };
    let scope = param("scope")
        .split(' ')
        .filter(|scope| knobs.grant_plan_scope || *scope != PLAN_SCOPE)
        .collect::<Vec<_>>()
        .join(" ");
    state.counter += 1;
    let code = format!("code-{}", state.counter);
    state.codes.insert(
        code.clone(),
        Grant {
            client_id,
            nonce: param("nonce"),
            scope: scope.clone(),
        },
    );
    {
        let mut pairs = callback.query_pairs_mut();
        pairs
            .append_pair("code", &code)
            .append_pair("scope", &scope)
            .append_pair("state", &param("state"));
        if let Some(client_id) = named {
            pairs.append_pair("client_id", &client_id);
        }
    }
    Redirect::to(callback.as_str()).into_response()
}

async fn token(
    State(server): State<Arc<AuthServerInner>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    server.record(TOKEN, &form);
    let field = |name: &str| form.get(name).cloned().unwrap_or_default();
    match field("grant_type").as_str() {
        "authorization_code" => {
            let mut state = server.state();
            let Some(grant) = state.codes.remove(&field("code")) else {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            };
            if grant.client_id != field("client_id") {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            }
            let id_token = server.id_token(&grant);
            state.counter += 1;
            let access = format!("access-{}", state.counter);
            let refresh = format!("refresh-{}", state.counter);
            state
                .live_refresh
                .insert(refresh.clone(), (grant.client_id, grant.scope.clone()));
            state.access_tokens.push(access.clone());
            state
                .issued
                .extend([access.clone(), refresh.clone(), id_token.clone()]);
            Json(json!({
                "access_token": access,
                "refresh_token": refresh,
                "id_token": id_token,
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": grant.scope,
            }))
            .into_response()
        }
        "refresh_token" => {
            // Long enough for a concurrent caller to arrive meanwhile.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut state = server.state();
            if state.knobs.revoked {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            }
            let Some((client_id, scope)) = state.live_refresh.remove(&field("refresh_token"))
            else {
                return oauth_error(400, json!({ "error": "invalid_grant" }));
            };
            state.counter += 1;
            let access = format!("access-{}", state.counter);
            let refresh = format!("refresh-{}", state.counter);
            state
                .live_refresh
                .insert(refresh.clone(), (client_id, scope.clone()));
            state.access_tokens.push(access.clone());
            state.issued.extend([access.clone(), refresh.clone()]);
            Json(json!({
                "access_token": access,
                "refresh_token": refresh,
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": scope,
            }))
            .into_response()
        }
        _ => oauth_error(400, json!({ "error": "unsupported_grant_type" })),
    }
}

async fn revoke(
    State(server): State<Arc<AuthServerInner>>,
    Form(form): Form<HashMap<String, String>>,
) -> StatusCode {
    server.record(REVOKE, &form);
    server.state().live_refresh.remove(&form["token"]);
    StatusCode::OK
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
            state: Mutex::new(AuthState::default()),
        });
        let app = Router::new()
            .route("/auth/.well-known/openid-configuration", get(discovery))
            .route("/auth/.well-known/jwks.json", get(jwks))
            .route(AUTHORIZE, get(authorize))
            .route(TOKEN, post(token))
            .route(REVOKE, post(revoke))
            .with_state(inner.clone());
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("the fake authorisation server runs");
        });
        Self { inner }
    }

    fn endpoints(&self) -> Endpoints {
        Endpoints::local(&self.inner.base)
    }

    fn knobs(&self, change: impl FnOnce(&mut Knobs)) {
        change(&mut self.inner.state().knobs);
    }

    fn seen(&self, path: &str) -> Vec<Seen> {
        self.inner
            .state()
            .seen
            .iter()
            .filter(|seen| seen.path == path)
            .cloned()
            .collect()
    }

    fn refreshes(&self) -> usize {
        self.seen(TOKEN)
            .iter()
            .filter(|seen| {
                seen.params.get("grant_type").map(String::as_str) == Some("refresh_token")
            })
            .count()
    }

    fn access_tokens(&self) -> Vec<String> {
        self.inner.state().access_tokens.clone()
    }

    fn issued(&self) -> Vec<String> {
        self.inner.state().issued.clone()
    }
}

// ---------------------------------------------------------------------------
// The ports
// ---------------------------------------------------------------------------

/// Records each URL. Pages on `127.0.0.1` are visited, following redirects,
/// as the user's browser would after the user approves; no other page is
/// fetched.
#[derive(Default)]
struct TestBrowser {
    opened: Mutex<Vec<Url>>,
    /// The user leaves the authorisation page open without answering.
    idle: Mutex<bool>,
}

impl TestBrowser {
    fn opened(&self) -> Vec<Url> {
        self.opened.lock().expect("browser lock poisoned").clone()
    }
}

impl Browser for TestBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        self.opened
            .lock()
            .expect("browser lock poisoned")
            .push(url.clone());
        let idle = *self.idle.lock().expect("browser lock poisoned");
        if url.host_str() == Some("127.0.0.1") && !idle {
            let url = url.clone();
            tokio::spawn(async move {
                let _ = reqwest::Client::new().get(url).send().await;
            });
        }
        Ok(())
    }
}

/// A secret store in memory whose deletes of chosen records fail.
#[derive(Default)]
struct Secrets {
    records: MemorySecretStore,
    failing_deletes: Mutex<HashSet<SecretRecord>>,
}

impl Secrets {
    fn get(&self, record: SecretRecord) -> Option<String> {
        self.records.get(record)
    }

    fn fail_deletes_of(&self, record: SecretRecord) {
        self.failing_deletes
            .lock()
            .expect("secrets lock poisoned")
            .insert(record);
    }
}

impl SecretStore for Secrets {
    fn read(&self, record: SecretRecord) -> Result<Option<String>, SecretError> {
        self.records.read(record)
    }

    fn write(&self, record: SecretRecord, value: &str) -> Result<(), SecretError> {
        self.records.write(record, value)
    }

    fn delete(&self, record: SecretRecord) -> Result<(), SecretError> {
        if self
            .failing_deletes
            .lock()
            .expect("secrets lock poisoned")
            .contains(&record)
        {
            return Err(SecretError(format!("{record:?} could not be deleted")));
        }
        self.records.delete(record)
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<CoreEvent>>);

impl Events {
    fn all(&self) -> Vec<CoreEvent> {
        self.0.lock().expect("events lock poisoned").clone()
    }

    fn sessions(&self) -> Vec<Session> {
        self.all()
            .into_iter()
            .filter_map(|event| match event {
                CoreEvent::SessionChanged { session } => Some(session),
                _ => None,
            })
            .collect()
    }
}

impl EventSink for Events {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().expect("events lock poisoned").push(event);
    }
}

// ---------------------------------------------------------------------------
// A provider that spends plan usage
// ---------------------------------------------------------------------------

/// How the plan route answers the next attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// The bearer token is refused.
    Token,
    /// The account or workspace may not use the plan.
    NotAllowed,
}

/// Behaves as a plan provider must towards its token source: a token before
/// each call; a refused token refreshed once, then reported rejected; a
/// not-allowed refusal reported. What it answers comes from a `FakeProvider`.
struct PlanProvider {
    answers: FakeProvider,
    tokens: OnceLock<Arc<dyn TokenSource>>,
    refusals: Mutex<VecDeque<Refusal>>,
    /// The bearer token of every attempt, in order.
    presented: Mutex<Vec<String>>,
}

impl PlanProvider {
    fn new() -> Self {
        Self {
            answers: FakeProvider::new(FakeScript::sample()),
            tokens: OnceLock::new(),
            refusals: Mutex::new(VecDeque::new()),
            presented: Mutex::new(Vec::new()),
        }
    }

    fn refuse(&self, refusals: impl IntoIterator<Item = Refusal>) {
        self.refusals
            .lock()
            .expect("provider lock poisoned")
            .extend(refusals);
    }

    fn presented(&self) -> Vec<String> {
        self.presented
            .lock()
            .expect("provider lock poisoned")
            .clone()
    }

    async fn authorise(&self) -> Result<(), ProviderError> {
        let tokens = self
            .tokens
            .get()
            .cloned()
            .expect("the core hands over the plan login's token source");
        let mut token = tokens.access_token().await?;
        let mut refreshed = false;
        loop {
            self.presented
                .lock()
                .expect("provider lock poisoned")
                .push(token.secret().to_owned());
            let refusal = self
                .refusals
                .lock()
                .expect("provider lock poisoned")
                .pop_front();
            match refusal {
                None => return Ok(()),
                Some(Refusal::NotAllowed) => {
                    tokens.plan_not_allowed().await;
                    return Err(refused());
                }
                Some(Refusal::Token) if !refreshed => {
                    refreshed = true;
                    token = tokens.refresh().await?;
                }
                Some(Refusal::Token) => {
                    tokens.rejected().await;
                    return Err(refused());
                }
            }
        }
    }
}

/// The error a refused call ends with. Which one is the provider's own
/// concern and specified with it; here only the session matters.
fn refused() -> ProviderError {
    ProviderError::Offline
}

#[async_trait]
impl ModelProvider for PlanProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        self.authorise().await?;
        self.answers.stream(request).await
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        self.authorise().await?;
        self.answers.list_models().await
    }

    fn use_plan_login(&self, tokens: Arc<dyn TokenSource>) -> bool {
        let _ = self.tokens.set(tokens);
        true
    }
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

struct Harness {
    server: FakeAuthServer,
    secrets: Arc<Secrets>,
    clock: Arc<FakeClock>,
    browser: Arc<TestBrowser>,
    events: Arc<Events>,
    plan: Arc<PlanProvider>,
    core: Core,
    /// Every response and error the core answered, as JSON.
    answers: Mutex<Vec<String>>,
    _dir: TempDir,
}

impl Harness {
    /// The core with a provider that spends plan usage.
    async fn plan() -> Self {
        let plan = Arc::new(PlanProvider::new());
        Self::with(plan.clone(), plan).await
    }

    /// The core with the fake provider, which spends no plan usage.
    async fn fake() -> Self {
        Self::with(
            Arc::new(FakeProvider::new(FakeScript::sample())),
            Arc::new(PlanProvider::new()),
        )
        .await
    }

    async fn with(provider: Arc<dyn ModelProvider>, plan: Arc<PlanProvider>) -> Self {
        let dir = tempfile::tempdir().expect("a temp dir");
        let clock = Arc::new(FakeClock::default());
        let server = FakeAuthServer::start(clock.clone()).await;
        let secrets = Arc::new(Secrets::default());
        let browser = Arc::new(TestBrowser::default());
        let events = Arc::new(Events::default());
        let core = Core::new(CoreDeps {
            database_path: dir.path().join("atlas.sqlite3"),
            provider,
            events: events.clone(),
            clock: clock.clone(),
            secrets: secrets.clone(),
            browser: browser.clone(),
            pdf: Arc::new(FakePdfExporter::default()),
            endpoints: server.endpoints(),
        })
        .expect("the core starts");
        Self {
            server,
            secrets,
            clock,
            browser,
            events,
            plan,
            core,
            answers: Mutex::new(Vec::new()),
            _dir: dir,
        }
    }

    async fn request(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        let result = self.core.handle(request).await;
        let json = match &result {
            Ok(response) => serde_json::to_string(response),
            Err(error) => serde_json::to_string(error),
        }
        .expect("answers serialise");
        self.answers
            .lock()
            .expect("answers lock poisoned")
            .push(json);
        result
    }

    async fn session_request(&self, request: CoreRequest) -> Session {
        match self.request(request).await {
            Ok(CoreResponse::Session { session }) => session,
            other => panic!("expected a session, got {other:?}"),
        }
    }

    async fn session(&self) -> Session {
        self.session_request(CoreRequest::GetSession).await
    }

    /// The session once no sign-in is waiting for the browser.
    async fn settled(&self) -> Session {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match self.session().await {
                    Session::SigningIn => tokio::time::sleep(Duration::from_millis(20)).await,
                    session => return session,
                }
            }
        })
        .await
        .expect("the sign-in settles")
    }

    async fn sign_in(&self) -> Session {
        let started = self.session_request(CoreRequest::SignIn).await;
        assert_eq!(started, Session::SigningIn);
        self.settled().await
    }

    async fn create_course(&self) -> Result<CoreResponse, CoreError> {
        self.request(CoreRequest::CreateCourse {
            topic: "Plate tectonics".to_owned(),
        })
        .await
    }

    async fn course_topics(&self) -> Vec<String> {
        match self.request(CoreRequest::ListCourses).await {
            Ok(CoreResponse::Courses { courses }) => {
                courses.into_iter().map(|course| course.topic).collect()
            }
            other => panic!("expected the courses, got {other:?}"),
        }
    }

    fn authorizations(&self) -> Vec<Seen> {
        self.server.seen(AUTHORIZE)
    }
}

fn signed_in(plan_usage: PlanUsage) -> Session {
    Session::SignedIn {
        account: Account {
            email: EMAIL.to_owned(),
        },
        plan_usage,
    }
}

fn signed_out(reason: Option<SignedOutReason>) -> Session {
    Session::SignedOut { reason }
}

// ---------------------------------------------------------------------------
// Signing in
// ---------------------------------------------------------------------------

#[tokio::test]
async fn first_launch_is_signed_out_and_a_completed_sign_in_has_plan_usage_on_and_the_email() {
    let harness = Harness::plan().await;

    assert_eq!(harness.session().await, signed_out(None));

    let session = harness.sign_in().await;

    assert_eq!(session, signed_in(PlanUsage::On));
    assert_eq!(
        harness.events.sessions(),
        vec![Session::SigningIn, signed_in(PlanUsage::On)]
    );
    assert_eq!(harness.authorizations().len(), 1);
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_some());
    assert!(harness.create_course().await.is_ok(), "plan usage is on");
}

#[tokio::test]
async fn a_sign_in_without_the_plan_permission_is_kept_with_plan_usage_off_until_enabled() {
    let harness = Harness::plan().await;
    harness.server.knobs(|knobs| knobs.grant_plan_scope = false);

    assert_eq!(harness.sign_in().await, signed_in(PlanUsage::Off));

    assert_eq!(
        harness.create_course().await,
        Err(CoreError::PlanUsageOff),
        "a request that spends plan usage is refused"
    );
    assert!(
        harness.plan.presented().is_empty(),
        "the refused request reaches no provider"
    );
    assert!(harness.course_topics().await.is_empty());

    harness.server.knobs(|knobs| knobs.grant_plan_scope = true);
    harness
        .request(CoreRequest::EnablePlanUsage)
        .await
        .expect("enabling starts");
    assert_eq!(harness.settled().await, signed_in(PlanUsage::On));
    let authorizations = harness.authorizations();
    assert_eq!(authorizations.len(), 2);
    assert_eq!(
        authorizations[1].params.get("prompt").map(String::as_str),
        Some("consent"),
        "enabling asks for consent again"
    );
    assert_eq!(
        harness.events.sessions().last(),
        Some(&signed_in(PlanUsage::On))
    );

    assert!(harness.create_course().await.is_ok(), "now it is accepted");
    assert_eq!(harness.course_topics().await, ["Plate tectonics"]);
}

#[tokio::test]
async fn with_the_fake_provider_the_guard_passes() {
    let harness = Harness::fake().await;

    assert!(
        harness.create_course().await.is_ok(),
        "signed out, the fake provider still answers"
    );

    harness.server.knobs(|knobs| knobs.grant_plan_scope = false);
    assert_eq!(harness.sign_in().await, signed_in(PlanUsage::Off));
    assert!(
        harness.create_course().await.is_ok(),
        "the fake provider spends no plan usage"
    );
}

#[tokio::test]
async fn a_declined_sign_in_ends_signed_out_with_the_reason() {
    let harness = Harness::plan().await;
    harness.server.knobs(|knobs| knobs.decline = true);

    let session = harness.sign_in().await;

    assert_eq!(session, signed_out(Some(SignedOutReason::Declined)));
    assert_eq!(harness.events.sessions().last(), Some(&session));
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
}

#[tokio::test]
async fn a_cancelled_sign_in_returns_to_signed_out_and_exchanges_nothing() {
    let harness = Harness::plan().await;
    *harness.browser.idle.lock().unwrap() = true;

    assert_eq!(
        harness.session_request(CoreRequest::SignIn).await,
        Session::SigningIn
    );
    assert_eq!(harness.session().await, Session::SigningIn);

    let session = harness.session_request(CoreRequest::CancelSignIn).await;

    assert_eq!(session, signed_out(None));
    assert_eq!(
        harness.events.sessions(),
        vec![Session::SigningIn, signed_out(None)]
    );
    assert!(harness.server.seen(TOKEN).is_empty());
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
}

// ---------------------------------------------------------------------------
// The token source
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "expects one presented token where the first generation of a launch also lists the models with the same token; the corrected twin is an_expired_access_token_is_refreshed_before_a_generation"]
async fn an_expired_access_token_is_refreshed_before_a_provider_call() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    let first = harness.server.access_tokens();
    assert_eq!(first.len(), 1);

    harness.clock.advance(Duration::from_secs(3600));
    harness
        .create_course()
        .await
        .expect("the course is created");

    assert_eq!(harness.server.refreshes(), 1);
    let tokens = harness.server.access_tokens();
    assert_eq!(tokens.len(), 2);
    assert_eq!(
        harness.plan.presented(),
        [tokens[1].clone()],
        "only the refreshed token was presented"
    );
    assert_eq!(harness.session().await, signed_in(PlanUsage::On));
}

#[tokio::test]
#[ignore = "expects two presented tokens where the first generation of a launch also lists the models with the same token; the corrected twin is concurrent_generations_with_an_expiring_token_share_one_refresh"]
async fn concurrent_calls_with_an_expiring_token_share_one_refresh() {
    let harness = Harness::plan().await;
    harness.sign_in().await;

    harness.clock.advance(Duration::from_secs(3590));
    let (first, second) = tokio::join!(harness.create_course(), harness.create_course());
    first.expect("the first course is created");
    second.expect("the second course is created");

    assert_eq!(harness.server.refreshes(), 1);
    let fresh = harness.server.access_tokens()[1].clone();
    assert_eq!(harness.plan.presented(), [fresh.clone(), fresh]);
}

#[tokio::test]
async fn a_revoked_login_ends_signed_out_and_the_courses_stay() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    harness.create_course().await.expect("a course is stored");

    harness.server.knobs(|knobs| knobs.revoked = true);
    harness.plan.refuse([Refusal::Token]);
    let result = harness.create_course().await;

    assert!(result.is_err(), "{result:?}");
    let ended = signed_out(Some(SignedOutReason::LoginEnded));
    assert_eq!(harness.session().await, ended);
    assert_eq!(harness.events.sessions().last(), Some(&ended));
    assert_eq!(harness.server.refreshes(), 1);
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
    assert!(
        harness.secrets.get(SecretRecord::IssuedClientId).is_some(),
        "the issued client id is kept"
    );
    assert_eq!(harness.course_topics().await, ["Plate tectonics"]);
    assert_eq!(
        harness.authorizations().len(),
        1,
        "no sign-in is started automatically"
    );
}

#[tokio::test]
async fn a_token_refused_again_after_one_refresh_ends_the_login() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    harness.create_course().await.expect("a course is stored");

    harness.plan.refuse([Refusal::Token, Refusal::Token]);
    let result = harness.create_course().await;

    assert!(result.is_err(), "{result:?}");
    assert_eq!(harness.server.refreshes(), 1, "one refresh, no more");
    let ended = signed_out(Some(SignedOutReason::LoginEnded));
    assert_eq!(harness.session().await, ended);
    assert_eq!(harness.events.sessions().last(), Some(&ended));
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
    assert_eq!(harness.course_topics().await, ["Plate tectonics"]);
}

#[tokio::test]
async fn after_a_not_allowed_report_the_session_stays_signed_in_as_not_allowed() {
    let harness = Harness::plan().await;
    harness.sign_in().await;

    harness.plan.refuse([Refusal::NotAllowed]);
    assert!(harness.create_course().await.is_err());

    let not_allowed = signed_in(PlanUsage::NotAllowed);
    assert_eq!(harness.session().await, not_allowed);
    assert_eq!(harness.events.sessions().last(), Some(&not_allowed));

    let presented = harness.plan.presented().len();
    assert_eq!(
        harness.create_course().await,
        Err(CoreError::PlanNotAllowed)
    );
    assert_eq!(
        harness.plan.presented().len(),
        presented,
        "the guard refuses before the provider"
    );
    assert_eq!(
        harness.request(CoreRequest::EnablePlanUsage).await,
        Err(CoreError::PlanNotAllowed)
    );
    assert_eq!(harness.session().await, not_allowed);
    assert_eq!(
        harness.authorizations().len(),
        1,
        "no further authorisation request is made"
    );
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_some());
}

#[tokio::test]
async fn only_a_new_explicit_sign_in_clears_not_allowed() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    harness.plan.refuse([Refusal::NotAllowed]);
    assert!(harness.create_course().await.is_err());
    assert_eq!(harness.session().await, signed_in(PlanUsage::NotAllowed));

    assert_eq!(harness.sign_in().await, signed_in(PlanUsage::On));

    assert_eq!(harness.authorizations().len(), 2);
    assert!(harness.create_course().await.is_ok());
}

impl Harness {
    /// Closes the app and opens it again over the same secret store, database
    /// and clock, with a new plan provider. `offline` leaves the
    /// authorisation server out of reach.
    async fn relaunch(&mut self, offline: bool) {
        let endpoints = if offline {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind a free port");
            let address = listener.local_addr().expect("the bound address");
            drop(listener);
            Endpoints::local(&Url::parse(&format!("http://{address}/")).expect("a valid URL"))
        } else {
            self.server.endpoints()
        };
        let plan = Arc::new(PlanProvider::new());
        self.core = Core::new(CoreDeps {
            database_path: self._dir.path().join("atlas.sqlite3"),
            provider: plan.clone(),
            events: self.events.clone(),
            clock: self.clock.clone(),
            secrets: self.secrets.clone(),
            browser: self.browser.clone(),
            pdf: Arc::new(FakePdfExporter::default()),
            endpoints,
        })
        .expect("the core starts again");
        self.plan = plan;
    }
}

#[tokio::test]
async fn not_allowed_is_remembered_with_the_login_across_a_relaunch() {
    let mut harness = Harness::plan().await;
    harness.sign_in().await;
    harness.plan.refuse([Refusal::NotAllowed]);
    assert!(harness.create_course().await.is_err());

    harness.relaunch(false).await;

    assert_eq!(harness.session().await, signed_in(PlanUsage::NotAllowed));
    assert_eq!(
        harness.create_course().await,
        Err(CoreError::PlanNotAllowed)
    );
    assert!(
        harness.plan.presented().is_empty(),
        "the guard refuses before the provider"
    );
    assert_eq!(
        harness.authorizations().len(),
        1,
        "no sign-in is started automatically"
    );

    assert_eq!(harness.sign_in().await, signed_in(PlanUsage::On));
    assert!(harness.create_course().await.is_ok());
}

#[tokio::test]
async fn an_expired_token_with_the_token_endpoint_out_of_reach_is_offline() {
    let mut harness = Harness::plan().await;
    harness.sign_in().await;
    harness.clock.advance(Duration::from_secs(3600));

    harness.relaunch(true).await;

    assert_eq!(harness.create_course().await, Err(CoreError::Offline));
    assert!(
        harness.plan.presented().is_empty(),
        "no stale token is sent"
    );
    assert_eq!(harness.session().await, signed_in(PlanUsage::On));
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_some());
}

#[tokio::test]
#[ignore = "expects one presented token where the first generation of a launch also lists the models with the same token; the corrected twin is a_token_about_to_expire_is_still_used_for_a_generation_when_the_token_endpoint_is_out_of_reach"]
async fn a_token_about_to_expire_is_still_used_when_the_token_endpoint_is_out_of_reach() {
    let mut harness = Harness::plan().await;
    harness.sign_in().await;
    let first = harness.server.access_tokens();
    harness.clock.advance(Duration::from_secs(3590));

    harness.relaunch(true).await;

    harness
        .create_course()
        .await
        .expect("the still valid token is used");
    assert_eq!(harness.plan.presented(), first);
    assert_eq!(harness.session().await, signed_in(PlanUsage::On));
}

impl Harness {
    /// Reads the launch's model list, so that a later generation is one
    /// provider call, and answers how many tokens were presented so far.
    async fn models_listed(&self) -> usize {
        self.request(CoreRequest::ListModels)
            .await
            .expect("the models are listed");
        self.plan.presented().len()
    }
}

#[tokio::test]
async fn an_expired_access_token_is_refreshed_before_a_generation() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    let listed = harness.models_listed().await;

    harness.clock.advance(Duration::from_secs(3600));
    harness
        .create_course()
        .await
        .expect("the course is created");

    assert_eq!(harness.server.refreshes(), 1);
    let tokens = harness.server.access_tokens();
    assert_eq!(tokens.len(), 2);
    let presented = harness.plan.presented();
    assert_eq!(
        presented[listed..],
        [tokens[1].clone()],
        "the generation presented only the refreshed token"
    );
    assert_eq!(harness.session().await, signed_in(PlanUsage::On));
}

#[tokio::test]
async fn concurrent_generations_with_an_expiring_token_share_one_refresh() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    let listed = harness.models_listed().await;

    harness.clock.advance(Duration::from_secs(3590));
    let (first, second) = tokio::join!(harness.create_course(), harness.create_course());
    first.expect("the first course is created");
    second.expect("the second course is created");

    assert_eq!(harness.server.refreshes(), 1);
    let fresh = harness.server.access_tokens()[1].clone();
    let presented = harness.plan.presented();
    assert_eq!(presented[listed..], [fresh.clone(), fresh]);
}

#[tokio::test]
async fn a_token_about_to_expire_is_still_used_for_a_generation_when_the_token_endpoint_is_out_of_reach()
 {
    let mut harness = Harness::plan().await;
    harness.sign_in().await;
    let first = harness.server.access_tokens();
    harness.clock.advance(Duration::from_secs(3590));

    harness.relaunch(true).await;
    // The listing itself goes out with the still valid token.
    let listed = harness.models_listed().await;

    harness
        .create_course()
        .await
        .expect("the still valid token is used");
    let presented = harness.plan.presented();
    assert_eq!(presented[..listed], first[..]);
    assert_eq!(presented[listed..], first[..]);
    assert_eq!(harness.session().await, signed_in(PlanUsage::On));
}

// ---------------------------------------------------------------------------
// Signing out
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sign_out_ends_the_plan_login_and_the_usage_login_together() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    harness
        .secrets
        .write(SecretRecord::UsageLogin, "usage-login-record")
        .expect("a usage login is present");
    let refresh_token = harness
        .server
        .issued()
        .into_iter()
        .find(|token| token.starts_with("refresh-"))
        .expect("a refresh token was issued");

    let session = harness.session_request(CoreRequest::SignOut).await;

    let out = signed_out(Some(SignedOutReason::SignedOut));
    assert_eq!(session, out);
    assert_eq!(harness.session().await, out);
    assert_eq!(harness.events.sessions().last(), Some(&out));
    let revocations = harness.server.seen(REVOKE);
    assert_eq!(revocations.len(), 1);
    assert_eq!(revocations[0].params["token"], refresh_token);
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
    assert!(
        harness.secrets.get(SecretRecord::IssuedClientId).is_some(),
        "the issued client id is kept"
    );
    assert!(harness.secrets.get(SecretRecord::UsageLogin).is_none());
    assert!(harness.events.all().contains(&CoreEvent::UsageChanged {
        usage: UsageState::NotConnected
    }));
    assert_eq!(
        harness.create_course().await,
        Err(CoreError::SignInRequired)
    );
}

#[tokio::test]
async fn a_failed_plan_sign_out_still_ends_the_usage_login() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    harness
        .secrets
        .write(SecretRecord::UsageLogin, "usage-login-record")
        .expect("a usage login is present");
    harness.secrets.fail_deletes_of(SecretRecord::PlanLogin);

    let result = harness.request(CoreRequest::SignOut).await;

    assert!(result.is_err(), "the failure is reported: {result:?}");
    assert!(harness.secrets.get(SecretRecord::UsageLogin).is_none());
}

#[tokio::test]
async fn a_failed_usage_sign_out_still_ends_the_plan_login() {
    let harness = Harness::plan().await;
    harness.sign_in().await;
    harness
        .secrets
        .write(SecretRecord::UsageLogin, "usage-login-record")
        .expect("a usage login is present");
    harness.secrets.fail_deletes_of(SecretRecord::UsageLogin);

    let result = harness.request(CoreRequest::SignOut).await;

    assert!(result.is_err(), "the failure is reported: {result:?}");
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
    assert_eq!(
        harness.session().await,
        signed_out(Some(SignedOutReason::SignedOut))
    );
}

// ---------------------------------------------------------------------------
// External links
// ---------------------------------------------------------------------------

#[tokio::test]
async fn manage_usage_opens_chatgpt_usage_settings() {
    let harness = Harness::plan().await;

    let response = harness.request(CoreRequest::OpenManageUsage).await;

    assert_eq!(response, Ok(CoreResponse::Done));
    assert_eq!(
        harness.browser.opened(),
        [Url::parse("https://chatgpt.com/settings/usage").unwrap()]
    );
}

#[tokio::test]
async fn open_external_opens_only_web_links() {
    let harness = Harness::plan().await;

    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "atlas://course/1",
        "vscode://file/etc/passwd",
        "mailto:ada@example.com",
        "not a link",
    ] {
        let result = harness
            .request(CoreRequest::OpenExternal {
                url: url.to_owned(),
            })
            .await;
        assert!(
            matches!(result, Err(CoreError::InvalidRequest { .. })),
            "{url} is refused, got {result:?}"
        );
    }
    assert!(harness.browser.opened().is_empty());

    for url in [
        "https://en.wikipedia.org/wiki/Plate_tectonics",
        "http://example.com/a",
    ] {
        let result = harness
            .request(CoreRequest::OpenExternal {
                url: url.to_owned(),
            })
            .await;
        assert_eq!(result, Ok(CoreResponse::Done), "{url} opens");
    }
    assert_eq!(
        harness.browser.opened(),
        [
            Url::parse("https://en.wikipedia.org/wiki/Plate_tectonics").unwrap(),
            Url::parse("http://example.com/a").unwrap(),
        ]
    );
}

// ---------------------------------------------------------------------------
// Tokens stay inside the core
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_response_or_event_contains_a_token() {
    let harness = Harness::plan().await;
    harness.server.knobs(|knobs| knobs.grant_plan_scope = false);
    harness.sign_in().await;
    harness.server.knobs(|knobs| knobs.grant_plan_scope = true);
    harness
        .request(CoreRequest::EnablePlanUsage)
        .await
        .expect("enabling starts");
    harness.settled().await;
    harness.clock.advance(Duration::from_secs(3600));
    harness.create_course().await.expect("a course is created");
    harness.plan.refuse([Refusal::NotAllowed]);
    let _ = harness.create_course().await;
    harness.sign_in().await;
    harness.plan.refuse([Refusal::Token, Refusal::Token]);
    let _ = harness.create_course().await;
    harness.sign_in().await;
    let _ = harness.request(CoreRequest::SignOut).await;

    let tokens = harness.server.issued();
    assert!(tokens.len() >= 9, "tokens were issued: {}", tokens.len());
    let mut seen = harness.answers.lock().unwrap().clone();
    seen.extend(
        harness
            .events
            .all()
            .iter()
            .map(|event| serde_json::to_string(event).expect("events serialise")),
    );
    assert!(seen.len() > 20);
    for text in &seen {
        for token in &tokens {
            assert!(
                !text.contains(token.as_str()),
                "a token reached the interface: {text}"
            );
        }
    }
}
