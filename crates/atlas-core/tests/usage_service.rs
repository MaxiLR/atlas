//! The usage meter through `Core::handle`, read with the plan login: a Codex
//! login seeded in a memory secret store, readings from a usage endpoint on
//! `127.0.0.1` that can also hang up, a fake clock and the fake model
//! provider.
//!
//! Readings on schedule are taken by the core on its own, so the tests move
//! the fake clock and then wait, in real time, for a reading to arrive or
//! for long enough that one would have.

mod support;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::Core;
use atlas_core::ports::{Endpoints, SecretRecord, SecretStore};
use atlas_core::protocol::{
    Account, CoreError, CoreEvent, CoreRequest, CoreResponse, PlanUsage, Session, UsageFailure,
    UsageLevel, UsageReading, UsageState, UsageWindow,
};
use atlas_core::provider::{ProviderError, Task};
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
use url::Url;
use uuid::Uuid;

const USAGE: &str = "/chatgpt/backend-api/wham/usage";
const ACCOUNT: &str = "acct-1";
const EMAIL: &str = "maxi@example.com";
const REFRESH: &str = "refresh-secret-1";

/// 12 November 2026, 09:00 UTC, as milliseconds since the epoch.
const NOW_MS: i64 = 1_794_474_000_000;

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

// -- the plan login --------------------------------------------------------------

/// The plan login's access token: an unsigned JWT carrying the account.
fn access() -> String {
    let claims = json!({
        "https://api.openai.com/auth": { "chatgpt_account_id": ACCOUNT },
        "https://api.openai.com/profile": { "email": EMAIL },
    });
    [
        URL_SAFE_NO_PAD.encode(json!({ "alg": "none" }).to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        "signature".to_owned(),
    ]
    .join(".")
}

/// A Codex login, as the app's sign-in stores it, valid for years.
fn sign_in(secrets: &dyn SecretStore) {
    let login = json!({
        "client_id": "app_EMoamEEZ73f0CkXaXp7hrann",
        "subject": "user-usage",
        "email": EMAIL,
        "access_token": access(),
        "refresh_token": REFRESH,
        "expires_at": "2030-01-01T00:00:00Z",
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

#[derive(Default)]
struct UsageScript {
    answers: VecDeque<Answer>,
    reads: Vec<Read>,
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

    fn reads(&self) -> Vec<Read> {
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
    let answer = {
        let mut script = script.lock().expect("lock");
        if method != "GET" || path != USAGE {
            Answer::Json(404, json!({ "error": "no such route" }))
        } else {
            script.reads.push(Read {
                authorization: header("authorization"),
                account: header("chatgpt-account-id"),
            });
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

    async fn with(signed_in: bool, usage: Vec<Answer>, provider: FakeProvider) -> Self {
        let usage = UsageServer::start(usage).await;
        let ports = FakePorts {
            clock: Arc::new(FakeClock::new(now())),
            endpoints: Endpoints {
                chatgpt: usage.url.join("chatgpt").expect("a valid URL"),
                ..FakePorts::default().endpoints
            },
            ..FakePorts::default()
        };
        if signed_in {
            sign_in(ports.secrets.as_ref());
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

    /// No event the core emitted carries a token of the plan login.
    fn assert_no_token(&self) {
        for event in self.events() {
            let shown = [
                format!("{event:?}"),
                serde_json::to_string(&event).expect("serialises"),
            ];
            for secret in [access(), REFRESH.to_owned()] {
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
async fn a_refused_reading_is_stale_and_never_ends_the_plan_login() {
    let harness = Harness::signed_in(vec![
        usage_ok(),
        Answer::Json(401, json!({ "error": "expired" })),
    ])
    .await;
    harness.first_reading().await;
    harness.focus(true).await;

    harness.advance(5 * MINUTE);
    let refused = connected(now(), Some(UsageFailure::Auth));
    harness
        .until("stale after a 401", |harness| {
            harness.last_state() == Some(refused.clone())
        })
        .await;

    assert!(harness.secrets.get(SecretRecord::PlanLogin).is_some());
    assert_eq!(
        harness.request(CoreRequest::GetSession).await,
        Ok(CoreResponse::Session {
            session: Session::SignedIn {
                account: Account {
                    email: EMAIL.to_owned()
                },
                plan_usage: PlanUsage::On,
            }
        })
    );
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
