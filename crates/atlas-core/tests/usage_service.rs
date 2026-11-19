//! The usage meter through `Core::handle`: its own device-code sign-in
//! against a scripted authorisation server on `127.0.0.1`, readings from a
//! usage endpoint on `127.0.0.1` that can also hang up, a fake clock, a
//! memory secret store and the fake model provider.
//!
//! Readings on schedule are taken by the core on its own, so the tests move
//! the fake clock and then wait, in real time, for a reading to arrive or
//! for long enough that one would have.

mod support;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::ports::{Endpoints, SecretRecord, SecretStore};
use atlas_core::protocol::{
    CoreError, CoreEvent, CoreRequest, CoreResponse, UsageFailure, UsageLevel, UsageReading,
    UsageState, UsageWindow,
};
use atlas_core::provider::{ProviderError, Task};
use atlas_core::store::Store;
use atlas_core::testing::{
    FakeClock, FakePorts, FakeProvider, FakeScript, MemorySecretStore, RecordingBrowser,
    RecordingEvents,
};
use atlas_core::{Core, CoreDeps};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use support::sample_course::{LAYERS, sample_course};
use support::server::{Reply, Route, TestServer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;
use uuid::Uuid;

const USERCODE: &str = "/auth/api/accounts/deviceauth/usercode";
const POLL: &str = "/auth/api/accounts/deviceauth/token";
const TOKEN: &str = "/auth/oauth/token";
const USAGE: &str = "/chatgpt/backend-api/wham/usage";

/// 12 November 2026, 09:00 UTC, as milliseconds since the epoch.
const NOW_MS: i64 = 1_794_474_000_000;
/// The device code's polling interval, in seconds.
const INTERVAL: i64 = 3;

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

/// The moment the login finishes: one poll after the code is shown.
fn signed_in_at() -> DateTime<Utc> {
    now() + TimeDelta::seconds(INTERVAL)
}

// -- tokens ------------------------------------------------------------------

/// An unsigned JWT for the account, told apart by `n`.
fn access(n: u32) -> String {
    let claims = json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-1",
            "chatgpt_plan_type": "plus",
        },
        "https://api.openai.com/profile": { "email": "maxi@example.com" },
        "n": n,
    });
    [
        URL_SAFE_NO_PAD.encode(json!({ "alg": "none" }).to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        "signature".to_owned(),
    ]
    .join(".")
}

fn refresh(n: u32) -> String {
    format!("refresh-secret-{n}")
}

fn tokens(n: u32, expires_in: u64) -> Value {
    json!({
        "access_token": access(n),
        "refresh_token": refresh(n),
        "expires_in": expires_in,
    })
}

/// Every secret a test's sign-in or refresh hands out.
fn secrets() -> Vec<String> {
    let mut secrets = vec!["auth-code-1".to_owned(), "verifier-1".to_owned()];
    for n in 1..=3 {
        secrets.push(access(n));
        secrets.push(refresh(n));
    }
    secrets
}

fn device_code(interval: i64) -> Value {
    json!({
        "device_auth_id": "device-1",
        "user_code": "ABCD-1234",
        "interval": interval.to_string(),
    })
}

fn approved() -> Value {
    json!({ "authorization_code": "auth-code-1", "code_verifier": "verifier-1" })
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
            resets_at: Some(at_ms(1_794_480_480_000)),
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

#[derive(Default)]
struct UsageScript {
    answers: VecDeque<Answer>,
    /// The `Authorization` header of each request, in order.
    reads: Vec<Option<String>>,
}

/// The usage endpoint on `127.0.0.1`, answering in order with the last answer
/// repeating, and recording every request. Stops when dropped.
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
            reads: Vec::new(),
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

    fn reads(&self) -> Vec<Option<String>> {
        self.script.lock().expect("lock").reads.clone()
    }
}

impl Drop for UsageServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(mut socket: tokio::net::TcpStream, script: Arc<Mutex<UsageScript>>) {
    let mut head = Vec::new();
    let mut chunk = [0_u8; 4096];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        match socket.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => head.extend_from_slice(&chunk[..read]),
        }
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let path = head
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or_default()
        .to_owned();
    let authorization = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("authorization")
            .then(|| value.trim().to_owned())
    });
    let answer = {
        let mut script = script.lock().expect("lock");
        if path != USAGE {
            Answer::Json(404, json!({ "error": "no such route" }))
        } else {
            script.reads.push(authorization);
            if script.answers.len() > 1 {
                script.answers.pop_front().expect("an answer is left")
            } else {
                script.answers[0].clone()
            }
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

// -- the harness ---------------------------------------------------------------

/// What OpenAI's authorisation server answers.
struct Auth {
    device: Value,
    polls: Vec<Reply>,
    /// The token exchange, then each refresh.
    tokens: Vec<Reply>,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            device: device_code(INTERVAL),
            polls: vec![Reply::json(200, &approved())],
            tokens: vec![Reply::json(200, &tokens(1, 3600))],
        }
    }
}

struct Harness {
    dir: tempfile::TempDir,
    auth: TestServer,
    usage: UsageServer,
    core: Core,
    clock: Arc<FakeClock>,
    secrets: Arc<MemorySecretStore>,
    browser: Arc<RecordingBrowser>,
    recorded: Arc<RecordingEvents>,
    events: Mutex<Vec<CoreEvent>>,
}

impl Harness {
    async fn start(auth: Auth, usage: Vec<Answer>) -> Self {
        Self::with(auth, usage, FakeProvider::new(FakeScript::sample())).await
    }

    async fn with(auth: Auth, usage: Vec<Answer>, provider: FakeProvider) -> Self {
        let server = TestServer::start([
            Route::post(USERCODE, [Reply::json(200, &auth.device)]),
            Route::post(POLL, auth.polls),
            Route::post(TOKEN, auth.tokens),
        ])
        .await;
        let usage = UsageServer::start(usage).await;
        let ports = FakePorts {
            clock: Arc::new(FakeClock::new(now())),
            endpoints: Endpoints {
                chatgpt: usage.url.join("chatgpt").expect("a valid URL"),
                ..server.endpoints()
            },
            ..FakePorts::default()
        };
        let dir = tempfile::tempdir().expect("a temp dir");
        let recorded = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(database(&dir), Arc::new(provider), recorded.clone()))
            .expect("core starts");
        Self {
            dir,
            auth: server,
            usage,
            core,
            clock: ports.clock,
            secrets: ports.secrets,
            browser: ports.browser,
            recorded,
            events: Mutex::new(Vec::new()),
        }
    }

    async fn request(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        self.core.handle(request).await
    }

    async fn meter(&self, request: CoreRequest) -> UsageState {
        match self.request(request).await {
            Ok(CoreResponse::Usage { usage }) => usage,
            other => panic!("expected the usage meter, got {other:?}"),
        }
    }

    async fn get(&self) -> UsageState {
        self.meter(CoreRequest::GetUsage).await
    }

    async fn focus(&self, focused: bool) {
        assert_eq!(
            self.request(CoreRequest::SetWindowFocused { focused })
                .await,
            Ok(CoreResponse::Done)
        );
    }

    /// Connects and waits for the reading taken right after.
    async fn connect(&self) {
        let shown = self.meter(CoreRequest::ConnectUsage).await;
        assert!(
            matches!(shown, UsageState::Connecting { .. }),
            "connecting: {shown:?}"
        );
        self.until("the first reading", |harness| harness.reads() == 1)
            .await;
        self.until("the reading is shown", |harness| {
            !matches!(
                harness.usage_states().last(),
                Some(UsageState::Connecting { .. })
            )
        })
        .await;
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

    fn reads(&self) -> usize {
        self.usage.reads().len()
    }

    fn auth_requests(&self, path: &str) -> usize {
        self.auth
            .requests()
            .iter()
            .filter(|request| request.path == path)
            .count()
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

    /// No event the core emitted carries a token or a code of the sign-in.
    fn assert_no_token(&self) {
        for event in self.events() {
            let shown = [
                format!("{event:?}"),
                serde_json::to_string(&event).expect("serialises"),
            ];
            for secret in secrets() {
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

// -- not connected -------------------------------------------------------------

#[tokio::test]
async fn before_connecting_the_meter_is_empty_and_the_usage_endpoint_is_not_read() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;

    assert_eq!(harness.get().await, UsageState::NotConnected);
    harness.focus(true).await;
    harness.advance(10 * MINUTE);
    harness.settle().await;
    harness.focus(false).await;
    harness.focus(true).await;
    harness.settle().await;

    assert_eq!(harness.get().await, UsageState::NotConnected);
    assert_eq!(harness.reads(), 0);
    assert!(harness.auth.requests().is_empty());
    assert!(harness.secrets.get(SecretRecord::UsageLogin).is_none());
}

// -- connecting ------------------------------------------------------------------

#[tokio::test]
async fn connecting_shows_the_code_and_the_page_then_takes_a_reading() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;
    let page = harness
        .auth
        .url()
        .join("auth/codex/device")
        .expect("a valid URL");
    let connecting = UsageState::Connecting {
        user_code: "ABCD-1234".to_owned(),
        verification_url: page.to_string(),
        expires_at: now() + TimeDelta::minutes(15),
    };

    assert_eq!(harness.meter(CoreRequest::ConnectUsage).await, connecting);
    harness
        .until("a reading", |harness| harness.reads() == 1)
        .await;
    harness
        .until("the reading is shown", |harness| {
            harness.usage_states().len() == 2
        })
        .await;

    let reading = connected(signed_in_at(), None);
    assert_eq!(harness.usage_states(), [connecting, reading.clone()]);
    assert_eq!(harness.get().await, reading);
    assert_eq!(harness.browser.opened(), [page]);
    assert_eq!(
        harness.usage.reads(),
        [Some(format!("Bearer {}", access(1)))]
    );
    assert!(
        harness.secrets.get(SecretRecord::UsageLogin).is_some(),
        "the usage login is stored under its own record"
    );
    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_none());
    harness.assert_no_token();
}

#[tokio::test]
async fn cancelling_the_connection_stops_the_polls() {
    let harness = Harness::start(
        Auth {
            // Pending for as long as it is asked, one second apart: the
            // 15 minutes take 900 polls.
            device: device_code(1),
            polls: vec![Reply::json(403, &json!({ "error": "pending" }))],
            ..Auth::default()
        },
        vec![usage_ok()],
    )
    .await;

    let shown = harness.meter(CoreRequest::ConnectUsage).await;
    assert!(matches!(shown, UsageState::Connecting { .. }), "{shown:?}");
    harness
        .until("a poll", |harness| harness.auth_requests(POLL) > 0)
        .await;

    assert_eq!(
        harness.meter(CoreRequest::CancelConnectUsage).await,
        UsageState::NotConnected
    );
    let polls = harness.auth_requests(POLL);
    harness.settle().await;

    assert_eq!(harness.auth_requests(POLL), polls, "polling went on");
    assert!(polls < 900, "the login ran to its deadline");
    assert_eq!(harness.auth_requests(TOKEN), 0);
    assert_eq!(harness.reads(), 0);
    assert_eq!(harness.get().await, UsageState::NotConnected);
    assert_eq!(harness.last_state(), Some(UsageState::NotConnected));
    assert!(harness.secrets.get(SecretRecord::UsageLogin).is_none());
    harness.assert_no_token();
}

// -- readings on schedule ------------------------------------------------------

#[tokio::test]
async fn with_focus_four_minutes_make_no_reading_and_five_make_one() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;
    harness.connect().await;
    harness.focus(true).await;

    harness.advance(4 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading at four minutes");

    harness.advance(MINUTE);
    harness
        .until("a reading at five minutes", |harness| harness.reads() == 2)
        .await;
    let taken = signed_in_at() + TimeDelta::minutes(5);
    harness
        .until("the new reading is shown", |harness| {
            harness.last_state() == Some(connected(taken, None))
        })
        .await;
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
    harness.assert_no_token();
}

#[tokio::test]
async fn without_focus_no_reading_and_one_when_focus_returns_after_five_minutes() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;
    harness.connect().await;
    harness.focus(false).await;

    harness.advance(6 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1, "a reading without focus");

    harness.focus(true).await;
    harness
        .until("a reading as focus returns", |harness| harness.reads() == 2)
        .await;

    // Focus lost and back within five minutes reads nothing more.
    harness.focus(false).await;
    harness.advance(MINUTE);
    harness.focus(true).await;
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
    harness.assert_no_token();
}

#[tokio::test]
async fn a_completed_generation_triggers_a_reading() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;
    let course = harness.lay_down_course();
    harness.connect().await;

    let opened = harness
        .request(CoreRequest::OpenSection {
            course_id: course,
            section_id: LAYERS,
        })
        .await;

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
    let harness = Harness::with(Auth::default(), vec![usage_ok()], provider).await;
    let course = harness.lay_down_course();
    harness.connect().await;

    let opened = harness
        .request(CoreRequest::OpenSection {
            course_id: course,
            section_id: LAYERS,
        })
        .await;

    assert!(opened.is_err(), "the generation fails: {opened:?}");
    harness.settle().await;
    assert_eq!(harness.reads(), 1);
}

// -- failed readings -------------------------------------------------------------

#[tokio::test]
async fn a_failed_reading_keeps_the_last_windows_stale_with_its_reason() {
    let harness = Harness::start(
        Auth::default(),
        vec![
            usage_ok(),
            Answer::HangUp,
            Answer::Json(500, json!({ "error": "server" })),
        ],
    )
    .await;
    harness.connect().await;
    harness.focus(true).await;
    assert_eq!(harness.get().await, connected(signed_in_at(), None));

    harness.advance(5 * MINUTE);
    let network = connected(signed_in_at(), Some(UsageFailure::Network));
    harness
        .until("stale after a network failure", |harness| {
            harness.last_state() == Some(network.clone())
        })
        .await;
    assert_eq!(harness.get().await, network);

    harness.advance(5 * MINUTE);
    let unexpected = connected(signed_in_at(), Some(UsageFailure::Unexpected));
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
async fn a_failed_first_reading_has_no_windows() {
    let harness = Harness::start(
        Auth::default(),
        vec![Answer::Json(500, json!({ "error": "server" }))],
    )
    .await;
    harness.connect().await;

    let failed = UsageState::FirstReadFailed {
        reason: UsageFailure::Unexpected,
    };
    assert_eq!(harness.last_state(), Some(failed.clone()));
    assert_eq!(harness.get().await, failed);
    harness.assert_no_token();
}

#[tokio::test]
async fn a_401_with_a_refused_refresh_ends_signed_out_with_no_windows() {
    let harness = Harness::start(
        Auth {
            tokens: vec![
                Reply::json(200, &tokens(1, 3600)),
                Reply::json(400, &json!({ "error": "invalid_grant" })),
            ],
            ..Auth::default()
        },
        vec![usage_ok(), Answer::Json(401, json!({ "error": "expired" }))],
    )
    .await;
    harness.connect().await;
    harness.focus(true).await;

    harness.advance(5 * MINUTE);
    harness
        .until("signed out", |harness| {
            harness.last_state() == Some(UsageState::SignedOut)
        })
        .await;

    assert_eq!(harness.get().await, UsageState::SignedOut);
    assert_eq!(harness.auth_requests(TOKEN), 2, "one refresh was tried");
    assert!(harness.secrets.get(SecretRecord::UsageLogin).is_none());
    // An ended login is not read again.
    harness.advance(10 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 2);
    harness.assert_no_token();
}

// -- the login -------------------------------------------------------------------

#[tokio::test]
async fn an_expiring_login_is_refreshed_and_stored_before_reading() {
    let harness = Harness::start(
        Auth {
            tokens: vec![
                Reply::json(200, &tokens(1, 600)),
                Reply::json(200, &tokens(2, 3600)),
            ],
            ..Auth::default()
        },
        vec![usage_ok()],
    )
    .await;
    harness.connect().await;
    let first = harness
        .secrets
        .get(SecretRecord::UsageLogin)
        .expect("the login is stored");
    harness.focus(true).await;

    harness.advance(5 * MINUTE);
    harness
        .until("a reading with the new token", |harness| {
            harness.reads() == 2
        })
        .await;

    assert_eq!(
        harness.usage.reads(),
        [
            Some(format!("Bearer {}", access(1))),
            Some(format!("Bearer {}", access(2))),
        ]
    );
    assert_eq!(harness.auth_requests(TOKEN), 2);
    let stored = harness
        .secrets
        .get(SecretRecord::UsageLogin)
        .expect("the login is still stored");
    assert_ne!(stored, first, "the refreshed login replaces the stored one");
    harness.assert_no_token();
}

#[tokio::test]
async fn a_stored_login_is_read_after_a_relaunch() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;
    harness.connect().await;
    let record = harness
        .secrets
        .get(SecretRecord::UsageLogin)
        .expect("the login is stored");

    let relaunched = Harness::start(Auth::default(), vec![usage_ok()]).await;
    relaunched
        .secrets
        .write(SecretRecord::UsageLogin, &record)
        .expect("the login is restored");
    relaunched.advance(10 * MINUTE);

    assert_eq!(
        relaunched.get().await,
        connected(now() + TimeDelta::minutes(10), None)
    );
    assert_eq!(
        relaunched.usage.reads(),
        [Some(format!("Bearer {}", access(1)))]
    );
    assert!(relaunched.auth.requests().is_empty(), "no new sign-in");
    relaunched.assert_no_token();
}

#[tokio::test]
async fn signing_out_ends_the_usage_login_and_its_readings() {
    let harness = Harness::start(Auth::default(), vec![usage_ok()]).await;
    harness.connect().await;
    harness.focus(true).await;

    let signed_out = harness.request(CoreRequest::SignOut).await;

    assert!(signed_out.is_ok(), "{signed_out:?}");
    assert_eq!(harness.get().await, UsageState::NotConnected);
    assert_eq!(harness.last_state(), Some(UsageState::NotConnected));
    assert!(harness.secrets.get(SecretRecord::UsageLogin).is_none());
    harness.advance(10 * MINUTE);
    harness.settle().await;
    assert_eq!(harness.reads(), 1);
}

// -- nothing else is affected ----------------------------------------------------

#[tokio::test]
async fn opening_a_section_completes_while_the_usage_endpoint_fails() {
    let harness = Harness::start(Auth::default(), vec![Answer::HangUp]).await;
    let course = harness.lay_down_course();
    harness.connect().await;
    harness.focus(true).await;

    let opened = harness
        .request(CoreRequest::OpenSection {
            course_id: course,
            section_id: LAYERS,
        })
        .await;

    match opened {
        Ok(CoreResponse::Section { section }) => assert_eq!(section.id, LAYERS),
        other => panic!("the section opens: {other:?}"),
    }
    harness
        .until("the reading after the generation", |harness| {
            harness.reads() == 2
        })
        .await;
    assert_eq!(
        harness.get().await,
        UsageState::FirstReadFailed {
            reason: UsageFailure::Network
        }
    );
    assert!(matches!(
        harness.request(CoreRequest::ListCourses).await,
        Ok(CoreResponse::Courses { .. })
    ));
    harness.assert_no_token();
}

#[tokio::test]
async fn a_usage_record_it_did_not_write_leaves_the_meter_empty() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let ports = FakePorts::default();
    ports
        .secrets
        .write(SecretRecord::UsageLogin, "usage-login-record")
        .expect("a record is present");
    let deps: CoreDeps = ports.deps(
        database(&dir),
        Arc::new(FakeProvider::new(FakeScript::sample())),
        Arc::new(RecordingEvents::default()),
    );
    let core = Core::new(deps).expect("core starts");

    assert_eq!(
        core.handle(CoreRequest::GetUsage).await,
        Ok(CoreResponse::Usage {
            usage: UsageState::NotConnected
        })
    );
    assert!(matches!(
        core.handle(CoreRequest::ListCourses).await,
        Ok(CoreResponse::Courses { .. })
    ));
}
