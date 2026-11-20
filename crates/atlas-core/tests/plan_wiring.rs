//! The ChatGPT plan provider wired to the session, through `Core::handle`: the
//! provider the shell builds, which takes its tokens from the plan login,
//! against one local server for the authorisation server and the plan route
//! (`/auth/…` and `/api/…` on `127.0.0.1`), with a secret store in memory, a
//! browser that follows only local redirects, and a fake clock.
//!
//! The Codex callback port is fixed, so each harness holds [`CALLBACK_PORT`]
//! and the tests run one at a time.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::ports::{Browser, BrowserError, Clock, Endpoints};
use atlas_core::protocol::{
    Account, CoreError, CoreEvent, CoreRequest, CoreResponse, Model, ModelList, PlanUsage, Session,
    SignedOutReason,
};
use atlas_core::protocol::{Goal, PlacementAnswer, PlacementState, PlacementStep, SelfRating};
use atlas_core::provider::{FakeProvider, FakeScript, Task};
use atlas_core::provider::{ProviderChoice, for_session};
use atlas_core::testing::FakePorts;
use atlas_core::testing::{FakeClock, FakePdfExporter, MemorySecretStore, RecordingEvents};
use atlas_core::{Core, CoreDeps};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::routing::{get, post};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::net::TcpListener;
use url::Url;
use uuid::Uuid;

const EMAIL: &str = "grace@example.com";
const SUBJECT: &str = "user-9Kd1";
const KEY_ID: &str = "wiring-key-1";

const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTHORIZE: &str = "/auth/oauth/authorize";
const TOKEN: &str = "/auth/oauth/token";
const RESPONSES: &str = "/api/responses";
const MODELS: &str = "/api/models";

/// Held by every harness: sign-in listens on the one registered port.
static CALLBACK_PORT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
// One local server: the authorisation server and the plan route
// ---------------------------------------------------------------------------

/// One scripted answer of the plan route.
#[derive(Debug, Clone)]
struct Reply {
    status: u16,
    content_type: &'static str,
    body: String,
}

impl Reply {
    fn json(status: u16, body: &Value) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.to_string(),
        }
    }

    fn sse(body: &str) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
            body: body.to_owned(),
        }
    }
}

/// A request to the plan route as the server received it.
#[derive(Debug, Clone)]
struct Call {
    path: String,
    bearer: Option<String>,
}

#[derive(Default)]
struct ServerState {
    /// The token endpoint's form of every call, in order.
    token_calls: Vec<HashMap<String, String>>,
    codes: HashSet<String>,
    /// Refresh tokens still valid.
    live_refresh: HashSet<String>,
    /// Every access token issued, in order.
    access_tokens: Vec<String>,
    counter: u32,
    /// The answers of `POST /api/responses`, in order; the last repeats.
    responses: VecDeque<Reply>,
    /// Every call to the plan route, in order.
    calls: Vec<Call>,
}

struct ServerInner {
    base: Url,
    clock: Arc<FakeClock>,
    state: Mutex<ServerState>,
}

impl ServerInner {
    fn issuer(&self) -> String {
        format!("{}auth", self.base)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ServerState> {
        self.state.lock().expect("server lock poisoned")
    }

    fn id_token(&self) -> String {
        let now = self.clock.now().timestamp();
        let claims = json!({
            "iss": self.issuer(),
            "sub": SUBJECT,
            "email": EMAIL,
            "aud": CODEX_CLIENT_ID,
            "iat": now,
            "exp": now + 3600,
        });
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KEY_ID.to_owned());
        let key = EncodingKey::from_rsa_pem(SIGNING_KEY.as_bytes()).expect("a valid test key");
        jsonwebtoken::encode(&header, &claims, &key).expect("a signed ID token")
    }

    /// Issues a fresh access and refresh token.
    fn issue(&self, state: &mut ServerState) -> (String, String) {
        state.counter += 1;
        let access = format!("access-{}", state.counter);
        let refresh = format!("refresh-{}", state.counter);
        state.live_refresh.insert(refresh.clone());
        state.access_tokens.push(access.clone());
        (access, refresh)
    }
}

fn oauth_error(error: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response()
}

/// Approves at once.
async fn authorize(
    State(server): State<Arc<ServerInner>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let param = |name: &str| params.get(name).cloned().unwrap_or_default();
    let mut callback = Url::parse(&param("redirect_uri")).expect("a redirect URI");
    let mut state = server.state();
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
    State(server): State<Arc<ServerInner>>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let field = |name: &str| form.get(name).cloned().unwrap_or_default();
    let mut state = server.state();
    state.token_calls.push(form.clone());
    if field("client_id") != CODEX_CLIENT_ID {
        return oauth_error("invalid_client");
    }
    match field("grant_type").as_str() {
        "authorization_code" => {
            if !state.codes.remove(&field("code")) {
                return oauth_error("invalid_grant");
            }
            let (access, refresh) = server.issue(&mut state);
            Json(json!({
                "access_token": access,
                "refresh_token": refresh,
                "id_token": server.id_token(),
                "token_type": "Bearer",
                "expires_in": 3600,
            }))
            .into_response()
        }
        "refresh_token" => {
            if !state.live_refresh.remove(&field("refresh_token")) {
                return oauth_error("invalid_grant");
            }
            let (access, refresh) = server.issue(&mut state);
            Json(json!({
                "access_token": access,
                "refresh_token": refresh,
                "token_type": "Bearer",
                "expires_in": 3600,
            }))
            .into_response()
        }
        _ => oauth_error("unsupported_grant_type"),
    }
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned)
}

fn reply(reply: Reply) -> Response {
    Response::builder()
        .status(StatusCode::from_u16(reply.status).expect("a valid status"))
        .header("content-type", reply.content_type)
        .header("x-request-id", "req_wiring")
        .body(Body::from(reply.body))
        .expect("a valid response")
}

async fn models(State(server): State<Arc<ServerInner>>, headers: HeaderMap) -> Response {
    server.state().calls.push(Call {
        path: MODELS.to_owned(),
        bearer: bearer(&headers),
    });
    reply(Reply::json(
        200,
        &json!({
            "models": [
                { "slug": "gpt-wired", "display_name": "GPT Wired", "visibility": "list" },
                { "slug": "gpt-hidden", "display_name": "Hidden", "visibility": "hide" },
                { "slug": "gpt-spare", "display_name": "GPT Spare", "visibility": "list" }
            ]
        }),
    ))
}

async fn responses(
    State(server): State<Arc<ServerInner>>,
    headers: HeaderMap,
    _body: Bytes,
) -> Response {
    let mut state = server.state();
    state.calls.push(Call {
        path: RESPONSES.to_owned(),
        bearer: bearer(&headers),
    });
    let next = if state.responses.len() > 1 {
        state.responses.pop_front()
    } else {
        state.responses.front().cloned()
    };
    reply(next.unwrap_or_else(|| Reply::sse(&completed())))
}

struct FakeServer {
    inner: Arc<ServerInner>,
}

impl FakeServer {
    async fn start(clock: Arc<FakeClock>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let inner = Arc::new(ServerInner {
            base: Url::parse(&format!("http://{address}/")).expect("a valid URL"),
            clock,
            state: Mutex::new(ServerState::default()),
        });
        let app = Router::new()
            .route(AUTHORIZE, get(authorize))
            .route(TOKEN, post(token))
            .route(MODELS, get(models))
            .route(RESPONSES, post(responses))
            .with_state(inner.clone());
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("the fake server runs");
        });
        Self { inner }
    }

    fn endpoints(&self) -> Endpoints {
        Endpoints::local(&self.inner.base)
    }

    /// The plan route answers each later generation with `replies` in order,
    /// the last repeating.
    fn answer_responses(&self, replies: impl IntoIterator<Item = Reply>) {
        self.inner.state().responses = replies.into_iter().collect();
    }

    fn access_tokens(&self) -> Vec<String> {
        self.inner.state().access_tokens.clone()
    }

    fn refreshes(&self) -> usize {
        self.inner
            .state()
            .token_calls
            .iter()
            .filter(|form| form.get("grant_type").map(String::as_str) == Some("refresh_token"))
            .count()
    }

    /// The bearer token of every call to `path`, in order.
    fn bearers(&self, path: &str) -> Vec<Option<String>> {
        self.inner
            .state()
            .calls
            .iter()
            .filter(|call| call.path == path)
            .map(|call| call.bearer.clone())
            .collect()
    }
}

/// A whole streamed response that completes.
fn completed() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/responses/completed.sse"
    );
    std::fs::read_to_string(path).expect("the completed stream fixture")
}

/// A whole streamed response whose text is `text`, in one delta.
fn streamed(text: &str) -> String {
    let delta = json!({
        "type": "response.output_text.delta",
        "sequence_number": 0,
        "item_id": "msg_wiring",
        "output_index": 0,
        "content_index": 0,
        "delta": text,
    });
    let completed = json!({
        "type": "response.completed",
        "sequence_number": 1,
        "response": { "id": "resp_wiring", "status": "completed", "output": [] },
    });
    format!(
        "event: response.output_text.delta\ndata: {delta}\n\n\
         event: response.completed\ndata: {completed}\n\n"
    )
}

/// A valid answer of the draft-ladder task: eight concepts.
fn ladder() -> String {
    let concepts: Vec<String> = (1..=8).map(|rung| format!("Concept {rung}")).collect();
    json!({ "concepts": concepts }).to_string()
}

/// A valid answer of the placement-question task: four options, the first
/// one right, each wrong one labelled.
fn placement_question(n: usize) -> String {
    json!({
        "stem": format!("Question {n}: what does a fluid do under shear?"),
        "options": [
            { "text": format!("Right option {n}"), "misconception": null },
            { "text": format!("Wrong option {n}.1"), "misconception": "label 1" },
            { "text": format!("Wrong option {n}.2"), "misconception": "label 2" },
            { "text": format!("Wrong option {n}.3"), "misconception": "label 3" },
        ],
        "key": 0,
    })
    .to_string()
}

/// Writes into `database` a course in placement whose ladder is drafted and
/// whose first question waits for its answer, with a core on the fake
/// provider, so the plan route sees only the generation under test. Returns
/// the course and the question.
async fn course_awaiting_an_answer(database: &Path) -> (Uuid, Uuid) {
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [Ok(ladder())])
        .answer(Task::PlacementQuestion, [Ok(placement_question(1))]);
    let core = Core::new(FakePorts::default().deps(
        database.to_owned(),
        Arc::new(provider),
        Arc::new(RecordingEvents::default()),
    ))
    .expect("the seeding core starts");
    let course_id = match core
        .handle(CoreRequest::CreateCourse {
            topic: "Fluid dynamics".to_owned(),
        })
        .await
    {
        Ok(CoreResponse::Course { course }) => course.id,
        other => panic!("expected a course, got {other:?}"),
    };
    match core
        .handle(CoreRequest::SubmitSelfRating {
            course_id,
            rating: SelfRating::NewToThis,
            goal: Goal::Overview,
        })
        .await
    {
        Ok(CoreResponse::Placement {
            placement:
                PlacementState {
                    step: PlacementStep::Question { question },
                    ..
                },
            ..
        }) => (course_id, question.id),
        other => panic!("expected the first question, got {other:?}"),
    }
}

fn fixture_error(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/errors/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("an error fixture"))
        .expect("an error fixture is JSON")
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

/// Visits each page on `127.0.0.1`, following redirects, as the user's
/// browser would after the user approves; no other page is fetched.
#[derive(Default)]
struct LocalBrowser;

impl Browser for LocalBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        if url.host_str() == Some("127.0.0.1") {
            let url = url.clone();
            tokio::spawn(async move {
                let _ = reqwest::Client::new().get(url).send().await;
            });
        }
        Ok(())
    }
}

struct Harness {
    server: FakeServer,
    clock: Arc<FakeClock>,
    events: Arc<RecordingEvents>,
    /// Every event taken from the sink so far.
    seen: Mutex<Vec<CoreEvent>>,
    core: Core,
    /// A course in placement whose first question waits for its answer.
    course_id: Uuid,
    question_id: Uuid,
    _dir: TempDir,
    /// Released when the test ends, for the next harness to sign in.
    _port: tokio::sync::MutexGuard<'static, ()>,
}

impl Harness {
    /// The core as the shell builds it by default: the plan provider on the
    /// server's endpoints, with no token source of its own.
    async fn start() -> Self {
        let port = CALLBACK_PORT.lock().await;
        callback_port_released().await;
        let dir = tempfile::tempdir().expect("a temp dir");
        let clock = Arc::new(FakeClock::default());
        let server = FakeServer::start(clock.clone()).await;
        let events = Arc::new(RecordingEvents::default());
        let endpoints = server.endpoints();
        let (course_id, question_id) =
            course_awaiting_an_answer(&dir.path().join("atlas.sqlite3")).await;
        // Each generation writes a valid next question unless a test scripts
        // the plan route otherwise.
        server.answer_responses([Reply::sse(&streamed(&placement_question(2)))]);
        let core = Core::new(CoreDeps {
            database_path: dir.path().join("atlas.sqlite3"),
            provider: for_session(ProviderChoice::Plan, &endpoints, clock.clone()),
            events: events.clone(),
            clock: clock.clone(),
            secrets: Arc::new(MemorySecretStore::default()),
            browser: Arc::new(LocalBrowser),
            pdf: Arc::new(FakePdfExporter::default()),
            endpoints,
        })
        .expect("the core starts");
        Self {
            server,
            clock,
            course_id,
            question_id,
            events,
            seen: Mutex::new(Vec::new()),
            core,
            _dir: dir,
            _port: port,
        }
    }

    async fn session(&self) -> Session {
        match self.core.handle(CoreRequest::GetSession).await {
            Ok(CoreResponse::Session { session }) => session,
            other => panic!("expected a session, got {other:?}"),
        }
    }

    /// Every event emitted so far, in order.
    fn events(&self) -> Vec<CoreEvent> {
        let mut seen = self.seen.lock().expect("events lock poisoned");
        seen.extend(self.events.take());
        seen.clone()
    }

    /// Waits until an emitted event matches `wanted` and returns it.
    async fn event(&self, wanted: impl Fn(&CoreEvent) -> bool) -> CoreEvent {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(event) = self.events().into_iter().find(|event| wanted(event)) {
                    return event;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the event is emitted")
    }

    /// Signs in through the browser and waits for the session to settle.
    async fn sign_in(&self) -> Session {
        match self.core.handle(CoreRequest::SignIn).await {
            Ok(CoreResponse::Session {
                session: Session::SigningIn,
            }) => {}
            other => panic!("expected the sign-in to start, got {other:?}"),
        }
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

    /// Signs in and waits for the model list the sign-in reads.
    async fn signed_in(&self) {
        assert_eq!(self.sign_in().await, signed_in(PlanUsage::On));
        self.event(|event| matches!(event, CoreEvent::ModelsChanged { .. }))
            .await;
    }

    /// A request that generates with the plan.
    /// It creates a course, a plan request that generates nothing, then
    /// answers the waiting placement question, which writes the next one: one
    /// generation. A refused request or a failed placement step answers its
    /// error.
    async fn generate(&self) -> Result<CoreResponse, CoreError> {
        self.core
            .handle(CoreRequest::CreateCourse {
                topic: "Fluid dynamics".to_owned(),
            })
            .await
            .map(drop)?;
        let response = self
            .core
            .handle(CoreRequest::AnswerPlacement {
                course_id: self.course_id,
                question_id: self.question_id,
                answer: PlacementAnswer::NotSure,
            })
            .await?;
        match &response {
            CoreResponse::Placement { placement, .. } => match &placement.error {
                Some(error) => Err(error.clone()),
                None => Ok(response),
            },
            other => panic!("expected a placement state, got {other:?}"),
        }
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

fn wired_list() -> ModelList {
    let model = |slug: &str, display_name: &str| Model {
        slug: slug.to_owned(),
        display_name: display_name.to_owned(),
    };
    ModelList {
        models: vec![
            model("gpt-wired", "GPT Wired"),
            model("gpt-spare", "GPT Spare"),
        ],
        current: Some("gpt-wired".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// The model list follows the session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn listing_models_before_sign_in_fails_as_sign_in_required() {
    let harness = Harness::start().await;

    assert_eq!(
        harness.core.handle(CoreRequest::ListModels).await,
        Err(CoreError::SignInRequired)
    );
    assert_eq!(
        harness.server.bearers(MODELS),
        Vec::<Option<String>>::new(),
        "no call reaches the plan route without a login"
    );
}

#[tokio::test]
async fn a_completed_sign_in_reports_the_plan_list_without_a_list_request() {
    let harness = Harness::start().await;

    assert_eq!(harness.sign_in().await, signed_in(PlanUsage::On));

    let event = harness
        .event(|event| matches!(event, CoreEvent::ModelsChanged { .. }))
        .await;
    assert_eq!(event, CoreEvent::ModelsChanged { list: wired_list() });
    let token = harness.server.access_tokens()[0].clone();
    assert_eq!(
        harness.server.bearers(MODELS),
        [Some(token)],
        "the list was read once, with the session's token"
    );
    let events = harness.events();
    let signed = events
        .iter()
        .position(|event| {
            event
                == &CoreEvent::SessionChanged {
                    session: signed_in(PlanUsage::On),
                }
        })
        .expect("the session change is reported");
    let listed = events
        .iter()
        .position(|event| matches!(event, CoreEvent::ModelsChanged { .. }))
        .expect("the list is reported");
    assert!(signed < listed, "the list follows the session: {events:?}");
}

// ---------------------------------------------------------------------------
// Generations carry the session's tokens
// ---------------------------------------------------------------------------

#[tokio::test]
async fn after_sign_in_a_generation_carries_the_session_access_token() {
    let harness = Harness::start().await;
    harness.signed_in().await;

    let _ = harness.generate().await;

    let token = harness.server.access_tokens()[0].clone();
    assert_eq!(harness.server.bearers(RESPONSES), [Some(token)]);
}

#[tokio::test]
async fn an_expired_token_is_refreshed_before_the_call() {
    let harness = Harness::start().await;
    harness.signed_in().await;

    harness.clock.advance(Duration::from_secs(3600));
    let _ = harness.generate().await;

    assert_eq!(harness.server.refreshes(), 1);
    let tokens = harness.server.access_tokens();
    assert_eq!(tokens.len(), 2);
    assert_eq!(
        harness.server.bearers(RESPONSES),
        [Some(tokens[1].clone())],
        "only the refreshed token was presented"
    );
    assert_eq!(harness.session().await, signed_in(PlanUsage::On));
}

#[tokio::test]
async fn a_refused_token_is_refreshed_once_and_a_second_refusal_ends_the_login() {
    let harness = Harness::start().await;
    harness.signed_in().await;
    let refused = Reply::json(401, &fixture_error("detail-401.json"));
    harness.server.answer_responses([refused.clone(), refused]);

    assert_eq!(harness.generate().await, Err(CoreError::SignInRequired));

    assert_eq!(harness.server.refreshes(), 1, "one refresh, no more");
    let tokens = harness.server.access_tokens();
    assert_eq!(
        harness.server.bearers(RESPONSES),
        [Some(tokens[0].clone()), Some(tokens[1].clone())]
    );
    let ended = Session::SignedOut {
        reason: Some(SignedOutReason::LoginEnded),
    };
    assert_eq!(harness.session().await, ended);
    assert!(
        harness
            .events()
            .contains(&CoreEvent::SessionChanged { session: ended }),
        "the ended login is reported"
    );
}

#[tokio::test]
async fn a_not_allowed_refusal_keeps_the_session_signed_in_as_not_allowed() {
    let harness = Harness::start().await;
    harness.signed_in().await;
    harness
        .server
        .answer_responses([Reply::json(403, &fixture_error("usage-not-included.json"))]);

    assert_eq!(harness.generate().await, Err(CoreError::PlanNotAllowed));

    let not_allowed = signed_in(PlanUsage::NotAllowed);
    assert_eq!(harness.session().await, not_allowed);
    assert!(
        harness.events().contains(&CoreEvent::SessionChanged {
            session: not_allowed
        }),
        "the not-allowed session is reported"
    );
    let calls = harness.server.bearers(RESPONSES).len();
    assert_eq!(calls, 1);

    assert_eq!(harness.generate().await, Err(CoreError::PlanNotAllowed));
    assert_eq!(
        harness.server.bearers(RESPONSES).len(),
        calls,
        "later plan requests are refused before the provider"
    );
}

// ---------------------------------------------------------------------------
// The model list follows the session out
// ---------------------------------------------------------------------------

/// The list a session with no plan to read reports.
fn no_list() -> ModelList {
    ModelList {
        models: Vec::new(),
        current: None,
    }
}

/// The model lists reported after the first event that matches `from`.
fn lists_after(events: &[CoreEvent], from: impl Fn(&CoreEvent) -> bool) -> Vec<ModelList> {
    let start = events
        .iter()
        .position(from)
        .expect("the starting event is reported");
    events[start..]
        .iter()
        .filter_map(|event| match event {
            CoreEvent::ModelsChanged { list } => Some(list.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn signing_out_reports_an_empty_model_list_without_reading_the_plan() {
    let harness = Harness::start().await;
    harness.signed_in().await;
    let reads = harness.server.bearers(MODELS).len();

    harness
        .core
        .handle(CoreRequest::SignOut)
        .await
        .expect("the sign-out succeeds");

    let out = Session::SignedOut {
        reason: Some(SignedOutReason::SignedOut),
    };
    assert_eq!(
        lists_after(&harness.events(), |event| event
            == &CoreEvent::SessionChanged {
                session: out.clone()
            }),
        [no_list()],
        "the old account's list is replaced once the session is out"
    );
    assert_eq!(
        harness.server.bearers(MODELS).len(),
        reads,
        "a signed-out session reads nothing from the plan route"
    );
    assert_eq!(
        harness.core.handle(CoreRequest::ListModels).await,
        Err(CoreError::SignInRequired)
    );
}

#[tokio::test]
async fn an_ended_login_reports_an_empty_model_list() {
    let harness = Harness::start().await;
    harness.signed_in().await;
    let refused = Reply::json(401, &fixture_error("detail-401.json"));
    harness.server.answer_responses([refused.clone(), refused]);

    assert_eq!(harness.generate().await, Err(CoreError::SignInRequired));

    harness
        .event(|event| event == &CoreEvent::ModelsChanged { list: no_list() })
        .await;
    let ended = Session::SignedOut {
        reason: Some(SignedOutReason::LoginEnded),
    };
    assert_eq!(
        lists_after(&harness.events(), |event| event
            == &CoreEvent::SessionChanged {
                session: ended.clone()
            }),
        [no_list()],
        "the ended login's list is replaced after the session is out"
    );
}
