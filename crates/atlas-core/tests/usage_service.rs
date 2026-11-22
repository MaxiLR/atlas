//! The usage meter through `Core::handle`, read with the plan login: a Codex
//! login seeded in a memory secret store, readings from a usage endpoint on
//! `127.0.0.1` that can also hang up, the login's token endpoint on the same
//! server, a fake clock and the fake model provider.
//!
//! Readings on schedule are taken by the core on its own, so the tests move
//! the fake clock and then wait, in real time, for a reading to arrive or
//! for long enough that one would have.

mod support;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use atlas_core::Core;
use atlas_core::ports::{Endpoints, SecretRecord, SecretStore};
use atlas_core::protocol::{
    Account, CoreError, CoreEvent, CoreRequest, CoreResponse, GenerationEvent, Model, PlanUsage,
    Session, UsageFailure, UsageLevel, UsageReading, UsageState, UsageWindow,
};
use atlas_core::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderStream, Task};
use atlas_core::store::Store;
use atlas_core::testing::{
    FakeClock, FakePorts, FakeProvider, FakeScript, MemorySecretStore, RecordingEvents,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use support::sample_course::{LAYERS, sample_course};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use url::Url;
use uuid::Uuid;

const USAGE: &str = "/chatgpt/backend-api/wham/usage";
const TOKEN: &str = "/auth/oauth/token";
const ACCOUNT: &str = "acct-1";
const EMAIL: &str = "maxi@example.com";
const REFRESH: &str = "refresh-secret-1";
/// The refresh token the token endpoint rotates to.
const FRESH_REFRESH: &str = "refresh-secret-2";

/// 12 November 2026, 09:00 UTC, as milliseconds since the epoch.
const NOW_MS: i64 = 1_794_474_000_000;
/// When the 5-hour window of `five-hour-weekly.json` resets.
const FIVE_HOUR_RESET_MS: i64 = 1_794_480_480_000;

const SECOND: Duration = Duration::from_secs(1);
const MINUTE: Duration = Duration::from_secs(60);
/// Long enough for a reading the core would take on its own to arrive.
const SETTLE: Duration = Duration::from_millis(2_500);
/// The longest a test waits for something that should happen.
const PATIENCE: Duration = Duration::from_secs(15);

fn at_ms(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).expect("a representable moment")
}

fn now() -> DateTime<Utc> {
    at_ms(NOW_MS)
}

/// `now()` moved on by `by`.
fn after(by: Duration) -> DateTime<Utc> {
    now() + TimeDelta::from_std(by).expect("a representable duration")
}

// -- the plan login --------------------------------------------------------------

/// An access token of the plan login: an unsigned JWT carrying the account,
/// told apart by its `signature`.
fn token_signed(signature: &str) -> String {
    let claims = json!({
        "https://api.openai.com/auth": { "chatgpt_account_id": ACCOUNT },
        "https://api.openai.com/profile": { "email": EMAIL },
    });
    [
        URL_SAFE_NO_PAD.encode(json!({ "alg": "none" }).to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        signature.to_owned(),
    ]
    .join(".")
}

/// The access token the login is stored with.
fn access() -> String {
    token_signed("signature")
}

/// The access token the token endpoint answers a refresh with.
fn fresh() -> String {
    token_signed("fresh-signature")
}

/// When a login that is never refreshed on its own expires.
fn lasting() -> DateTime<Utc> {
    "2030-01-01T00:00:00Z".parse().expect("a valid moment")
}

/// A Codex login, as the app's sign-in stores it, expiring at `expires_at`.
fn sign_in(secrets: &dyn SecretStore, expires_at: DateTime<Utc>) {
    let login = json!({
        "client_id": "app_EMoamEEZ73f0CkXaXp7hrann",
        "subject": "user-usage",
        "email": EMAIL,
        "access_token": access(),
        "refresh_token": REFRESH,
        "expires_at": expires_at,
    });
    secrets
        .write(SecretRecord::PlanLogin, &login.to_string())
        .expect("the login is stored");
}

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/usage/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture exists"))
        .expect("the fixture is JSON")
}

/// The windows `five-hour-weekly.json` reads as.
fn windows() -> Vec<UsageWindow> {
    vec![
        UsageWindow {
            name: "7d".to_owned(),
            length_seconds: 604_800,
            used: 0.41,
            resets_at: Some(at_ms(1_794_841_200_000)),
            level: UsageLevel::Normal,
        },
        UsageWindow {
            name: "5h".to_owned(),
            length_seconds: 18_000,
            used: 0.62,
            resets_at: Some(at_ms(FIVE_HOUR_RESET_MS)),
            level: UsageLevel::Normal,
        },
    ]
}

fn connected(taken_at: DateTime<Utc>, stale: Option<UsageFailure>) -> UsageState {
    UsageState::Connected {
        reading: UsageReading {
            windows: windows(),
            taken_at,
            stale,
        },
    }
}

// -- the usage endpoint ------------------------------------------------------

/// One answer of the usage endpoint.
#[derive(Debug, Clone)]
enum Answer {
    Json(u16, Value),
    /// Closes the connection without answering: a network failure.
    HangUp,
}

fn usage_ok() -> Answer {
    Answer::Json(200, fixture("five-hour-weekly"))
}

/// `five-hour-weekly.json` with its 5-hour window `used_percent` full and
/// resetting at `resets_at`, the limit reached or not.
fn usage_with(used_percent: u32, resets_at: DateTime<Utc>, limit_reached: bool) -> Answer {
    let mut usage = fixture("five-hour-weekly");
    usage["rate_limit"]["limit_reached"] = json!(limit_reached);
    let five_hour = &mut usage["rate_limit"]["primary_window"];
    five_hour["used_percent"] = json!(used_percent);
    five_hour["reset_at"] = json!(resets_at.timestamp());
    five_hour["reset_after_seconds"] = json!((resets_at - now()).num_seconds());
    Answer::Json(200, usage)
}

fn refused() -> Answer {
    Answer::Json(401, json!({ "error": "expired" }))
}

/// The headers of one request to the usage endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Read {
    authorization: Option<String>,
    account: Option<String>,
}

fn with_the_plan_login() -> Read {
    Read {
        authorization: Some(format!("Bearer {}", access())),
        account: Some(ACCOUNT.to_owned()),
    }
}

fn with_the_fresh_token() -> Read {
    Read {
        authorization: Some(format!("Bearer {}", fresh())),
        account: Some(ACCOUNT.to_owned()),
    }
}

/// One request the server received.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Hit {
    /// A reading of the usage endpoint.
    Usage(Read),
    /// A refresh at the token endpoint, with the refresh token it sent.
    Refresh(Option<String>),
}

#[derive(Default)]
struct UsageScript {
    answers: VecDeque<Answer>,
    hits: Vec<Hit>,
}

/// The usage endpoint and the token endpoint on `127.0.0.1`. The usage
/// endpoint answers in order with the last answer repeating; the token
/// endpoint always refreshes to `fresh()`. Every request is recorded. Stops
/// when dropped.
struct UsageServer {
    url: Url,
    script: Arc<Mutex<UsageScript>>,
    task: tokio::task::JoinHandle<()>,
}

impl UsageServer {
    async fn start(answers: Vec<Answer>) -> Self {
        assert!(!answers.is_empty(), "the usage endpoint has no answer");
        let script = Arc::new(Mutex::new(UsageScript {
            answers: answers.into(),
            hits: Vec::new(),
        }));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let url = Url::parse(&format!(
            "http://{}/",
            listener.local_addr().expect("the bound address")
        ))
        .expect("a valid URL");
        let shared = script.clone();
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve(socket, shared.clone()));
            }
        });
        Self { url, script, task }
    }

    /// Every request so far, in order.
    fn hits(&self) -> Vec<Hit> {
        self.script.lock().expect("lock").hits.clone()
    }

    /// The readings of the usage endpoint so far, in order.
    fn reads(&self) -> Vec<Read> {
        self.hits()
            .into_iter()
            .filter_map(|hit| match hit {
                Hit::Usage(read) => Some(read),
                Hit::Refresh(_) => None,
            })
            .collect()
    }
}

impl Drop for UsageServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(mut socket: tokio::net::TcpStream, script: Arc<Mutex<UsageScript>>) {
    let mut received = Vec::new();
    let mut chunk = [0_u8; 4096];
    let end_of_head = loop {
        if let Some(at) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break at + 4;
        }
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => received.extend_from_slice(&chunk[..read]),
        }
    };
    let head = String::from_utf8_lossy(&received[..end_of_head]).into_owned();
    let mut first = head.lines().next().unwrap_or_default().split(' ');
    let method = first.next().unwrap_or_default().to_owned();
    let path = first.next().unwrap_or_default().to_owned();
    let header = |wanted: &str| {
        head.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case(wanted)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length")
        .and_then(|length| length.parse().ok())
        .unwrap_or(0);
    while received.len() < end_of_head + length {
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => received.extend_from_slice(&chunk[..read]),
        }
    }
    let body = &received[end_of_head..end_of_head + length];
    let answer = {
        let mut script = script.lock().expect("lock");
        match (method.as_str(), path.as_str()) {
            ("GET", USAGE) => {
                script.hits.push(Hit::Usage(Read {
                    authorization: header("authorization"),
                    account: header("chatgpt-account-id"),
                }));
                if script.answers.len() > 1 {
                    script.answers.pop_front().expect("an answer is left")
                } else {
                    script.answers[0].clone()
                }
            }
            ("POST", TOKEN) => {
                let refresh_token = url::form_urlencoded::parse(body)
                    .find(|(name, _)| name == "refresh_token")
                    .map(|(_, value)| value.into_owned());
                script.hits.push(Hit::Refresh(refresh_token));
                Answer::Json(
                    200,
                    json!({
                        "access_token": fresh(),
                        "refresh_token": FRESH_REFRESH,
                        "expires_in": 3600,
                    }),
                )
            }
            _ => Answer::Json(404, json!({ "error": "no such route" })),
        }
    };
    match answer {
        Answer::HangUp => drop(socket),
        Answer::Json(status, body) => {
            let body = body.to_string();
            let response = format!(
                "HTTP/1.1 {status} Scripted\r\ncontent-type: application/json\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    }
}

// -- a provider holding a generation -------------------------------------------

/// The fake provider, holding the writing of each section until the test
/// releases it.
struct Holding {
    inner: FakeProvider,
    release: Arc<Semaphore>,
}

#[async_trait]
impl ModelProvider for Holding {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        if request.task == Task::WriteSection {
            self.release
                .acquire()
                .await
                .expect("the gate stays open")
                .forget();
        }
        self.inner.stream(request).await
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        self.inner.list_models().await
    }
}

// -- the harness ---------------------------------------------------------------

struct Harness {
    dir: tempfile::TempDir,
    usage: UsageServer,
    core: Core,
    clock: Arc<FakeClock>,
    secrets: Arc<MemorySecretStore>,
    recorded: Arc<RecordingEvents>,
    events: Mutex<Vec<CoreEvent>>,
}

impl Harness {
    /// The core with a plan login kept from an earlier launch.
    async fn signed_in(usage: Vec<Answer>) -> Self {
        Self::with(true, usage, FakeProvider::new(FakeScript::sample())).await
    }

    /// The core with no plan login.
    async fn signed_out(usage: Vec<Answer>) -> Self {
        Self::with(false, usage, FakeProvider::new(FakeScript::sample())).await
    }

    async fn with(
        signed_in: bool,
        usage: Vec<Answer>,
        provider: impl ModelProvider + 'static,
    ) -> Self {
        Self::build(signed_in.then(lasting), usage, provider).await
    }

    /// The core with a plan login expiring at `login`, if any.
    async fn build(
        login: Option<DateTime<Utc>>,
        usage: Vec<Answer>,
        provider: impl ModelProvider + 'static,
    ) -> Self {
        let usage = UsageServer::start(usage).await;
        let ports = FakePorts {
            clock: Arc::new(FakeClock::new(now())),
            endpoints: Endpoints {
                chatgpt: usage.url.join("chatgpt").expect("a valid URL"),
                auth: usage.url.join("auth").expect("a valid URL"),
                ..FakePorts::default().endpoints
            },
            ..FakePorts::default()
        };
        if let Some(expires_at) = login {
            sign_in(ports.secrets.as_ref(), expires_at);
        }
        let dir = tempfile::tempdir().expect("a temp dir");
        let recorded = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(database(&dir), Arc::new(provider), recorded.clone()))
            .expect("core starts");
        Self {
            dir,
            usage,
            core,
            clock: ports.clock,
            secrets: ports.secrets,
            recorded,
            events: Mutex::new(Vec::new()),
        }
    }

    async fn request(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        self.core.handle(request).await
    }

    async fn get(&self) -> UsageState {
        match self.request(CoreRequest::GetUsage).await {
            Ok(CoreResponse::Usage { usage }) => usage,
            other => panic!("expected the usage meter, got {other:?}"),
        }
    }

    async fn focus(&self, focused: bool) {
        assert_eq!(
            self.request(CoreRequest::SetWindowFocused { focused })
                .await,
            Ok(CoreResponse::Done)
        );
    }

    /// The first reading, taken by the first ask.
    async fn first_reading(&self) {
        assert_eq!(self.get().await, connected(now(), None));
        assert_eq!(self.reads(), 1);
    }

    /// The first reading, taken by the first ask; answers the level of its
    /// 5-hour window.
    async fn first_five_hour_level(&self) -> UsageLevel {
        let UsageState::Connected { reading } = self.get().await else {
            panic!("the first reading is shown");
        };
        assert_eq!(self.reads(), 1);
        reading
            .windows
            .iter()
            .find(|window| window.name == "5h")
            .expect("a 5-hour window")
            .level
    }

    /// Every event so far, in order.
    fn events(&self) -> Vec<CoreEvent> {
        let mut events = self.events.lock().expect("lock");
        events.extend(self.recorded.take());
        events.clone()
    }

    fn usage_states(&self) -> Vec<UsageState> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                CoreEvent::UsageChanged { usage } => Some(usage),
                _ => None,
            })
            .collect()
    }

    fn last_state(&self) -> Option<UsageState> {
        self.usage_states().pop()
    }

    /// When the reading shown was taken, and why it is stale if it is.
    fn shown(&self) -> Option<(DateTime<Utc>, Option<UsageFailure>)> {
        match self.last_state()? {
            UsageState::Connected { reading } => Some((reading.taken_at, reading.stale)),
            _ => None,
        }
    }

    /// Whether a generation has started.
    fn generation_started(&self) -> bool {
        self.events().iter().any(|event| {
            matches!(
                event,
                CoreEvent::Generation {
                    event: GenerationEvent::Started,
                    ..
                }
            )
        })
    }

    fn reads(&self) -> usize {
        self.usage.reads().len()
    }

    fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// Waits, in real time, until `done` holds.
    async fn until(&self, what: &str, done: impl Fn(&Self) -> bool) {
        let started = std::time::Instant::now();
        while !done(self) {
            assert!(started.elapsed() < PATIENCE, "never happened: {what}");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Waits long enough for a reading the core would take on its own.
    async fn settle(&self) {
        tokio::time::sleep(SETTLE).await;
    }

    /// The sample course laid down in the core's database; returns its id.
    fn lay_down_course(&self) -> Uuid {
        let store = Store::open(&database(&self.dir), self.clock.clone()).expect("store opens");
        let sample = sample_course();
        let course = store.create_course(&sample.topic).expect("a course").id;
        store
            .insert_index(course, &sample.index, &sample.map, &[])
            .expect("the index");
        course
    }

    /// Opens a section of the sample course, which generates it.
    async fn open_section(&self, course: Uuid) -> Result<CoreResponse, CoreError> {
        self.request(CoreRequest::OpenSection {
            course_id: course,
            section_id: LAYERS,
        })
        .await
    }

    /// The session is signed in with plan usage on, its login kept.
    async fn assert_signed_in(&self) {
        assert!(self.secrets.get(SecretRecord::PlanLogin).is_some());
        assert_eq!(
            self.request(CoreRequest::GetSession).await,
            Ok(CoreResponse::Session {
                session: Session::SignedIn {
                    account: Account {
                        email: EMAIL.to_owned()
                    },
                    plan_usage: PlanUsage::On,
                }
            })
        );
    }

    /// No event the core emitted carries a token of the plan login.
    fn assert_no_token(&self) {
        for event in self.events() {
            let shown = [
                format!("{event:?}"),
                serde_json::to_string(&event).expect("serialises"),
            ];
            for secret in [
                access(),
                fresh(),
                REFRESH.to_owned(),
                FRESH_REFRESH.to_owned(),
            ] {
                for text in &shown {
                    assert!(!text.contains(&secret), "an event leaks a secret: {text}");
                }
            }
        }
    }
}

fn database(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("atlas.sqlite3")
}

// -- no plan login -------------------------------------------------------------

#[tokio::test]
async fn signed_out_the_meter_has_no_plan_login_and_the_usage_endpoint_is_not_read() {
    let harness = Harness::signed_out(vec![usage_ok()]).await;

    assert_eq!(harness.get().await, UsageState::NoPlanLogin);
    harness.focus(true).await;
    harness.advance(6 * MINUTE);
    harness.settle().await;
    harness.focus(false).await;
    harness.focus(true).await;
    harness.settle().await;

    assert_eq!(harness.get().await, UsageState::NoPlanLogin);
    assert_eq!(harness.reads(), 0);
}

// -- a login kept from an earlier launch -----------------------------------------

#[tokio::test]
async fn a_kept_login_is_read_by_the_first_ask_with_the_plan_logins_token() {
    let harness = Harness::signed_in(vec![usage_ok()]).await;

    assert_eq!(harness.get().await, connected(now(), None));

    assert_eq!(harness.usage.reads(), [with_the_plan_login()]);
    assert_eq!(harness.last_state(), Some(connected(now(), None)));
    // A reading is there: the next ask answers it as it stands.
    harness.advance(MINUTE);
    assert_eq!(harness.get().await, connected(now(), None));
    assert_eq!(harness.reads(), 1);
    harness.assert_no_token();
}

#[tokio::test]
async fn after_a_failed_first_reading_the_next_ask_reads_again() {
    let harness = Harness::signed_in(vec![
        Answer::Json(500, json!({ "error": "server" })),
        usage_ok(),
    ])
    .await;

    assert_eq!(
        harness.get().await,
        UsageState::FirstReadFailed {
            reason: UsageFailure::Unexpected
        }
    );
    assert_eq!(harness.get().await, connected(now(), None));
    assert_eq!(harness.reads(), 2);
    harness.assert_no_token();
}

// -- signing out -----------------------------------------------------------------

#[tokio::test]
async fn signing_out_empties_the_meter_and_stops_its_readings() {
    let harness = Harness::signed_in(vec![usage_ok()]).await;
    let course = harness.lay_down_course();
    harness.first_reading().await;
    harness.focus(true).await;

    let signed_out = harness.request(CoreRequest::SignOut).await;

    assert!(signed_out.is_ok(), "{signed_out:?}");
    assert_eq!(harness.last_state(), Some(UsageState::NoPlanLogin));
    assert_eq!(harness.get().await, UsageState::NoPlanLogin);
    harness.advance(10 * MINUTE);
    harness.settle().await;
    let opened = harness.open_section(course).await;
    assert!(
        matches!(opened, Ok(CoreResponse::Section { .. })),
        "{opened:?}"
    );
    harness.settle().await;
    assert_eq!(harness.reads(), 1);
    assert_eq!(harness.get().await, UsageState::NoPlanLogin);
}

// -- readings on schedule ------------------------------------------------------

#[tokio::test]
async fn with_focus_four_minutes_make_no_reading_and_five_make_one() {
    let harness = Harness::signed_in(vec![usage_ok()]).await;
    harness.first_reading().await;
    harness.focus(true).await;

    harness.advance(4 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading at four minutes");

    harness.advance(MINUTE);
    harness
        .until("a reading at five minutes", |harness| harness.reads() == 2)
        .await;
    let taken = now() + TimeDelta::minutes(5);
    harness
        .until("the new reading is shown", |harness| {
            harness.last_state() == Some(connected(taken, None))
        })
        .await;
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
    assert_eq!(
        harness.usage.reads(),
        [with_the_plan_login(), with_the_plan_login()]
    );
    harness.assert_no_token();
}

#[tokio::test]
async fn without_focus_no_reading_and_one_when_focus_returns_after_five_minutes() {
    let harness = Harness::signed_in(vec![usage_ok()]).await;
    harness.first_reading().await;
    harness.focus(false).await;

    harness.advance(6 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading without focus");

    harness.focus(true).await;
    harness
        .until("a reading as focus returns", |harness| harness.reads() == 2)
        .await;

    // Focus lost and back within 30 seconds reads nothing more.
    harness.focus(false).await;
    harness.advance(20 * SECOND);
    harness.focus(true).await;
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
    harness.assert_no_token();
}

#[tokio::test]
async fn a_completed_generation_triggers_a_reading() {
    let harness = Harness::signed_in(vec![usage_ok()]).await;
    let course = harness.lay_down_course();
    harness.first_reading().await;

    let opened = harness.open_section(course).await;

    assert!(
        matches!(opened, Ok(CoreResponse::Section { .. })),
        "{opened:?}"
    );
    harness
        .until("a reading after the generation", |harness| {
            harness.reads() == 2
        })
        .await;
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
    harness.assert_no_token();
}

#[tokio::test]
async fn a_failed_generation_triggers_no_reading() {
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::WriteSection, [Err(ProviderError::Offline)]);
    let harness = Harness::with(true, vec![usage_ok()], provider).await;
    let course = harness.lay_down_course();
    harness.first_reading().await;

    let opened = harness.open_section(course).await;

    assert!(opened.is_err(), "the generation fails: {opened:?}");
    harness.settle().await;
    assert_eq!(harness.reads(), 1);
}

// -- failed readings -------------------------------------------------------------

#[tokio::test]
async fn a_failed_reading_keeps_the_last_windows_stale_with_its_reason() {
    let harness = Harness::signed_in(vec![
        usage_ok(),
        Answer::HangUp,
        Answer::Json(500, json!({ "error": "server" })),
    ])
    .await;
    harness.first_reading().await;
    harness.focus(true).await;

    harness.advance(5 * MINUTE);
    let network = connected(now(), Some(UsageFailure::Network));
    harness
        .until("stale after a network failure", |harness| {
            harness.last_state() == Some(network.clone())
        })
        .await;
    assert_eq!(harness.get().await, network);

    harness.advance(5 * MINUTE);
    let unexpected = connected(now(), Some(UsageFailure::Unexpected));
    harness
        .until("stale after a 500", |harness| {
            harness.last_state() == Some(unexpected.clone())
        })
        .await;
    assert_eq!(harness.get().await, unexpected);
    assert_eq!(harness.reads(), 3);
    harness.assert_no_token();
}

#[tokio::test]
async fn a_reading_refused_twice_is_stale_with_auth_and_never_ends_the_plan_login() {
    let harness = Harness::signed_in(vec![usage_ok(), refused()]).await;
    harness.first_reading().await;
    harness.focus(true).await;

    harness.advance(5 * MINUTE);
    let refused = connected(now(), Some(UsageFailure::Auth));
    harness
        .until("stale after a 401", |harness| {
            harness.last_state() == Some(refused.clone())
        })
        .await;

    // Refused, refreshed once, read once more with the fresh token, refused
    // again: a failed reading, and nothing more.
    harness.settle().await;
    assert_eq!(
        harness.usage.hits(),
        [
            Hit::Usage(with_the_plan_login()),
            Hit::Usage(with_the_plan_login()),
            Hit::Refresh(Some(REFRESH.to_owned())),
            Hit::Usage(with_the_fresh_token()),
        ]
    );
    assert_eq!(harness.get().await, refused);
    harness.assert_signed_in().await;
    harness.assert_no_token();
}

// -- nothing else is affected ----------------------------------------------------

#[tokio::test]
async fn opening_a_section_completes_while_the_usage_endpoint_fails() {
    let harness = Harness::signed_in(vec![Answer::HangUp]).await;
    let course = harness.lay_down_course();
    let failed = UsageState::FirstReadFailed {
        reason: UsageFailure::Network,
    };
    assert_eq!(harness.get().await, failed);
    harness.focus(true).await;

    let opened = harness.open_section(course).await;

    match opened {
        Ok(CoreResponse::Section { section }) => assert_eq!(section.id, LAYERS),
        other => panic!("the section opens: {other:?}"),
    }
    harness
        .until("the reading after the generation", |harness| {
            harness.reads() >= 2
        })
        .await;
    assert_eq!(harness.get().await, failed);
    assert!(matches!(
        harness.request(CoreRequest::ListCourses).await,
        Ok(CoreResponse::Courses { .. })
    ));
    harness.assert_no_token();
}

// -- tokens ----------------------------------------------------------------------

#[tokio::test]
async fn an_expiring_token_is_refreshed_before_the_reading() {
    let harness = Harness::build(
        Some(now() + TimeDelta::minutes(1)),
        vec![usage_ok()],
        FakeProvider::new(FakeScript::sample()),
    )
    .await;

    assert_eq!(harness.get().await, connected(now(), None));

    assert_eq!(
        harness.usage.hits(),
        [
            Hit::Refresh(Some(REFRESH.to_owned())),
            Hit::Usage(with_the_fresh_token()),
        ]
    );
    harness.assert_signed_in().await;
    harness.assert_no_token();
}

#[tokio::test]
async fn a_refused_reading_is_read_once_more_with_a_refreshed_token() {
    let harness = Harness::signed_in(vec![refused(), usage_ok()]).await;

    assert_eq!(harness.get().await, connected(now(), None));

    assert_eq!(
        harness.usage.hits(),
        [
            Hit::Usage(with_the_plan_login()),
            Hit::Refresh(Some(REFRESH.to_owned())),
            Hit::Usage(with_the_fresh_token()),
        ]
    );
    assert_eq!(harness.last_state(), Some(connected(now(), None)));
    harness.assert_signed_in().await;
    harness.assert_no_token();
}

// -- the pace while active ---------------------------------------------------------

#[tokio::test]
async fn while_a_generation_runs_a_reading_every_minute() {
    let release = Arc::new(Semaphore::new(0));
    let provider = Holding {
        inner: FakeProvider::new(FakeScript::sample()),
        release: release.clone(),
    };
    let harness = Harness::with(true, vec![usage_ok()], provider).await;
    let course = harness.lay_down_course();
    harness.first_reading().await;
    harness.focus(true).await;

    let opening = harness.open_section(course);
    let watching = async {
        harness
            .until("the generation starts", Harness::generation_started)
            .await;
        harness.advance(MINUTE);
        harness
            .until("a reading a minute into the generation", |harness| {
                harness.reads() == 2
            })
            .await;
        harness.advance(MINUTE);
        harness
            .until("a reading two minutes into the generation", |harness| {
                harness.reads() == 3
            })
            .await;
        release.add_permits(1);
    };
    let (opened, ()) = tokio::join!(opening, watching);

    assert!(
        matches!(opened, Ok(CoreResponse::Section { .. })),
        "{opened:?}"
    );
    harness.assert_no_token();
}

#[tokio::test]
async fn after_a_reading_that_changed_the_numbers_a_reading_every_minute() {
    let harness = Harness::signed_in(vec![
        usage_ok(),
        usage_with(65, at_ms(FIVE_HOUR_RESET_MS), false),
    ])
    .await;
    harness.first_reading().await;
    harness.focus(true).await;

    harness.advance(5 * MINUTE);
    harness
        .until("a changed reading at five minutes", |harness| {
            harness.shown() == Some((after(5 * MINUTE), None))
        })
        .await;

    harness.advance(MINUTE);
    harness
        .until("a reading a minute after the change", |harness| {
            harness.shown() == Some((after(6 * MINUTE), None))
        })
        .await;
    assert_eq!(harness.reads(), 3);

    // That reading changed nothing: back to five minutes apart.
    harness.advance(MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 3, "a reading a minute after no change");
    harness.assert_no_token();
}

#[tokio::test]
async fn with_a_window_from_three_quarters_full_a_reading_every_minute() {
    let harness = Harness::signed_in(vec![usage_with(75, at_ms(FIVE_HOUR_RESET_MS), false)]).await;
    assert_eq!(harness.first_five_hour_level().await, UsageLevel::High);
    harness.focus(true).await;

    harness.advance(MINUTE);
    harness
        .until("a reading at one minute", |harness| harness.reads() == 2)
        .await;
    harness.advance(MINUTE);
    harness
        .until("a reading at two minutes", |harness| harness.reads() == 3)
        .await;
    harness.settle().await;
    assert_eq!(harness.reads(), 3);
}

#[tokio::test]
async fn with_a_window_at_its_limit_readings_stay_five_minutes_apart() {
    let harness = Harness::signed_in(vec![usage_with(100, at_ms(FIVE_HOUR_RESET_MS), true)]).await;
    assert_eq!(harness.first_five_hour_level().await, UsageLevel::Limit);
    harness.focus(true).await;

    harness.advance(MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading at one minute");

    harness.advance(4 * MINUTE);
    harness
        .until("a reading at five minutes", |harness| harness.reads() == 2)
        .await;
}

#[tokio::test]
async fn after_a_failed_reading_none_before_five_minutes_even_while_active() {
    let high = usage_with(75, at_ms(FIVE_HOUR_RESET_MS), false);
    let harness = Harness::signed_in(vec![high.clone(), Answer::HangUp, high]).await;
    assert_eq!(harness.first_five_hour_level().await, UsageLevel::High);
    harness.focus(true).await;

    harness.advance(MINUTE);
    harness
        .until("stale after a network failure", |harness| {
            harness.shown() == Some((now(), Some(UsageFailure::Network)))
        })
        .await;

    harness.advance(4 * MINUTE - SECOND);
    harness.settle().await;
    assert_eq!(
        harness.reads(),
        2,
        "a reading within five minutes of a failure"
    );

    harness.advance(SECOND);
    harness
        .until("a reading five minutes after the failure", |harness| {
            harness.shown() == Some((after(6 * MINUTE), None))
        })
        .await;
    assert_eq!(harness.reads(), 3);
}

// -- resets and focus ------------------------------------------------------------

#[tokio::test]
async fn a_reading_when_a_windows_reset_time_passes() {
    let resets_at = now() + TimeDelta::seconds(150);
    let harness = Harness::signed_in(vec![usage_with(62, resets_at, false)]).await;
    assert_eq!(harness.first_five_hour_level().await, UsageLevel::Normal);
    harness.focus(true).await;

    harness.advance(2 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading before the reset");

    harness.advance(MINUTE);
    harness
        .until("a reading as the reset time passes", |harness| {
            harness.reads() == 2
        })
        .await;

    // The reset is behind the last reading: nothing more until five minutes.
    harness.advance(MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
}

#[tokio::test]
async fn focus_back_under_30_seconds_after_a_reading_reads_nothing_and_later_reads_once() {
    let harness = Harness::signed_in(vec![usage_ok()]).await;
    harness.first_reading().await;
    harness.focus(true).await;
    harness.focus(false).await;

    harness.advance(29 * SECOND);
    harness.focus(true).await;
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading as focus returns after 29 s");

    harness.focus(false).await;
    harness.advance(SECOND);
    harness.focus(true).await;
    harness
        .until("a reading as focus returns after 30 s", |harness| {
            harness.reads() == 2
        })
        .await;
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
}

#[tokio::test]
async fn without_focus_an_active_meter_reads_nothing_as_time_passes() {
    let resets_at = now() + TimeDelta::seconds(150);
    let harness = Harness::signed_in(vec![usage_with(75, resets_at, false)]).await;
    assert_eq!(harness.first_five_hour_level().await, UsageLevel::High);

    harness.advance(MINUTE);
    harness.settle().await;
    harness.advance(5 * MINUTE);
    harness.settle().await;

    assert_eq!(harness.reads(), 1);
}

#[tokio::test]
async fn a_fixed_clock_with_focus_takes_no_extra_readings() {
    let harness = Harness::signed_in(vec![usage_with(75, at_ms(FIVE_HOUR_RESET_MS), false)]).await;
    assert_eq!(harness.first_five_hour_level().await, UsageLevel::High);
    harness.focus(true).await;

    harness.settle().await;
    harness.settle().await;

    assert_eq!(harness.reads(), 1);
}
