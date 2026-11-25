//! The usage reader against a scripted server on `127.0.0.1` with a fake
//! clock: every reading case of Headroom's `codex-read.test.mjs` and
//! `levels.test.mjs`, with the same inputs, the same expected values and its
//! fixtures in `tests/fixtures/usage/`, read with the plan login's token.

mod support;

use std::sync::Arc;

use atlas_core::auth::AccessToken;
use atlas_core::ports::Endpoints;
use atlas_core::protocol::{UsageFailure, UsageLevel};
use atlas_core::testing::FakeClock;
use atlas_core::usage::{Usage, UsageReader, UsageReaderDeps, Window, level_of};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use support::server::{RecordedRequest, Reply, Route, TestServer};
use url::Url;

const USAGE: &str = "/chatgpt/backend-api/wham/usage";

/// 12 November 2026, 09:00 UTC, as milliseconds since the epoch.
const NOW_MS: i64 = 1_794_474_000_000;

fn at_ms(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).expect("a representable moment")
}

fn now() -> DateTime<Utc> {
    at_ms(NOW_MS)
}

/// An unsigned JWT whose payload is `claims`.
fn jwt(claims: &Value) -> String {
    [
        URL_SAFE_NO_PAD.encode(json!({ "alg": "none" }).to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string()),
        "signature".to_owned(),
    ]
    .join(".")
}

/// The plan login's access token, carrying its account and residency.
fn access() -> String {
    jwt(&json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-1",
            "chatgpt_plan_type": "plus",
            "chatgpt_data_residency": "eu",
        },
        "https://api.openai.com/profile": { "email": "maxi@example.com" },
    }))
}

fn token() -> AccessToken {
    AccessToken::new(access())
}

/// `base` with `fields` replaced, a `null` field removed.
fn with(base: Value, fields: Value) -> Value {
    let mut object = base.as_object().expect("an object").clone();
    for (key, value) in fields.as_object().expect("an object") {
        if value.is_null() {
            object.remove(key);
        } else {
            object.insert(key.clone(), value.clone());
        }
    }
    Value::Object(object)
}

fn ok(body: &Value) -> Reply {
    Reply::json(200, body)
}

struct Harness {
    server: TestServer,
    reader: UsageReader,
}

impl Harness {
    async fn start(routes: Vec<Route>) -> Self {
        let server = TestServer::start(routes).await;
        let endpoints = server.endpoints();
        Self::on(server, endpoints)
    }

    fn on(server: TestServer, endpoints: Endpoints) -> Self {
        let reader = UsageReader::new(UsageReaderDeps {
            endpoints,
            clock: Arc::new(FakeClock::new(now())),
        });
        Self { server, reader }
    }

    /// A harness whose every endpoint refuses the connection.
    async fn offline() -> Self {
        let server = TestServer::start([]).await;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a free port");
        let closed = Url::parse(&format!(
            "http://{}/",
            listener.local_addr().expect("the bound address")
        ))
        .expect("a valid URL");
        drop(listener);
        Self::on(server, Endpoints::local(&closed))
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.server.requests()
    }
}

/// A failure as it could reach a log or the interface carries none of
/// `secrets`.
fn assert_no_secret(failure: &UsageFailure, secrets: &[String]) {
    let shown = [
        format!("{failure:?}"),
        serde_json::to_string(failure).expect("serialises"),
    ];
    for secret in secrets {
        for text in &shown {
            assert!(!text.contains(secret.as_str()), "leaks a secret: {text}");
        }
    }
}

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/usage/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture exists"))
        .expect("the fixture is JSON")
}

/// Reads the usage once, answered with `reply`, at NOW.
async fn read_with(reply: Reply) -> (Harness, Result<Usage, UsageFailure>) {
    let harness = Harness::start(vec![Route::get(USAGE, [reply])]).await;
    let result = harness.reader.read(&token()).await;
    (harness, result)
}

async fn answering(name: &str) -> Result<Usage, UsageFailure> {
    read_with(ok(&fixture(name))).await.1
}

fn window(
    id: &str,
    label: &str,
    length_seconds: u64,
    used: f64,
    resets_at_ms: Option<i64>,
    at_limit: bool,
) -> Window {
    Window {
        id: id.to_owned(),
        label: label.to_owned(),
        length_seconds,
        used,
        resets_at: resets_at_ms.map(at_ms),
        at_limit,
    }
}

// Read.

#[tokio::test]
async fn reads_the_usage_with_the_headers_of_the_plan_route() {
    let (harness, _) = read_with(ok(&fixture("five-hour-weekly"))).await;
    let requests = harness.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.path, USAGE);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {}", access()).as_str())
    );
    assert_eq!(request.header("chatgpt-account-id"), Some("acct-1"));
    assert_eq!(
        request.header("x-openai-internal-codex-residency"),
        Some("eu")
    );
    assert_eq!(request.header("originator"), Some("atlas"));
    assert_eq!(request.header("version"), Some("0.169.0"));
    assert_eq!(
        request.header("user-agent"),
        Some(concat!("atlas/", env!("CARGO_PKG_VERSION")))
    );
}

#[tokio::test]
async fn a_5_hour_and_a_weekly_window_weekly_first() {
    assert_eq!(
        answering("five-hour-weekly").await,
        Ok(Usage {
            plan: Some("Plus".to_owned()),
            windows: vec![
                window(
                    "7d",
                    "Weekly",
                    604_800,
                    0.41,
                    Some(1_794_841_200_000),
                    false
                ),
                window("5h", "5-hour", 18_000, 0.62, Some(1_794_480_480_000), false),
            ],
        }),
    );
}

#[tokio::test]
async fn a_weekly_only_plan_reports_its_week_in_the_primary_slot() {
    assert_eq!(
        answering("weekly-only").await,
        Ok(Usage {
            plan: Some("Free".to_owned()),
            windows: vec![window("7d", "Weekly", 604_800, 0.0, None, false)],
        }),
    );
}

#[tokio::test]
async fn a_reached_limit_marks_the_fullest_window() {
    assert_eq!(
        answering("limit-reached").await,
        Ok(Usage {
            plan: Some("Pro".to_owned()),
            windows: vec![
                window(
                    "7d",
                    "Weekly",
                    604_800,
                    0.74,
                    Some(1_794_700_000_000),
                    false
                ),
                window("5h", "5-hour", 18_000, 0.985, Some(1_794_475_800_000), true),
            ],
        }),
    );
}

#[tokio::test]
async fn a_reached_limit_marks_every_window_at_100_percent_clamped_to_1() {
    assert_eq!(
        answering("limit-reached-full").await,
        Ok(Usage {
            plan: Some("Plus".to_owned()),
            windows: vec![
                window("7d", "Weekly", 604_800, 1.0, Some(1_794_700_000_000), true),
                window("5h", "5-hour", 18_000, 1.0, Some(1_794_475_800_000), true),
            ],
        }),
    );
}

#[tokio::test]
async fn additional_rate_limits_follow_the_general_windows() {
    assert_eq!(
        answering("additional").await,
        Ok(Usage {
            plan: Some("Pro".to_owned()),
            windows: vec![
                window("7d", "Weekly", 604_800, 0.1, Some(1_794_900_000_000), false),
                window("5h", "5-hour", 18_000, 0.2, Some(1_794_480_000_000), false),
                window(
                    "extra:gpt-5-3-codex-spark:7d",
                    "GPT-5.3-Codex-Spark · Weekly",
                    604_800,
                    0.3,
                    Some(1_794_900_000_000),
                    false,
                ),
                window(
                    "extra:gpt-5-3-codex-spark:5h",
                    "GPT-5.3-Codex-Spark · 5-hour",
                    18_000,
                    1.0,
                    Some(1_794_477_600_000),
                    true,
                ),
                window(
                    "extra:review-tasks:90m",
                    "Review Tasks · 90-minute",
                    5_400,
                    0.12,
                    Some(1_794_474_600_000),
                    false,
                ),
            ],
        }),
    );
}

#[tokio::test]
async fn an_unknown_window_length_is_named_by_its_length() {
    assert_eq!(
        answering("unknown-length").await,
        Ok(Usage {
            plan: None,
            windows: vec![
                window("1d", "1-day", 86_400, 0.25, Some(1_794_520_800_000), false),
                window("3h", "3-hour", 10_800, 0.5, Some(1_794_477_600_000), false),
            ],
        }),
    );
}

#[tokio::test]
async fn a_401_is_a_refused_token() {
    let (_, result) = read_with(Reply::json(
        401,
        &json!({ "error": format!("bad token {}", access()) }),
    ))
    .await;
    assert_eq!(result, Err(UsageFailure::Auth));
    assert_no_secret(&result.expect_err("fails"), &[access()]);
}

#[tokio::test]
async fn a_rejected_read_fetch_is_a_network_failure() {
    let harness = Harness::offline().await;
    let result = harness.reader.read(&token()).await;
    assert_eq!(result, Err(UsageFailure::Network));
    assert_no_secret(&result.expect_err("fails"), &[access()]);
}

#[tokio::test]
async fn a_500_is_unexpected_and_never_carries_its_body() {
    let (_, result) = read_with(Reply::text(
        500,
        &format!("internal error for Bearer {}", access()),
    ))
    .await;
    assert_eq!(result, Err(UsageFailure::Unexpected));
    assert_no_secret(&result.expect_err("fails"), &[access()]);
}

#[tokio::test]
async fn a_body_that_is_not_an_object_is_unexpected() {
    for body in ["\"ok\"", "[1,2]", "null"] {
        let (_, result) = read_with(Reply::new(200, "application/json", body)).await;
        assert_eq!(result, Err(UsageFailure::Unexpected), "{body}");
    }
}

#[tokio::test]
async fn the_plan_reads_as_words() {
    async fn plan_of(plan_type: &str) -> Option<String> {
        let body = with(
            fixture("five-hour-weekly"),
            json!({ "plan_type": plan_type }),
        );
        read_with(ok(&body)).await.1.expect("reads").plan
    }
    assert_eq!(
        plan_of("self_serve_business_prolite").await.as_deref(),
        Some("Self serve business prolite"),
    );
    assert_eq!(plan_of("plus").await.as_deref(), Some("Plus"));
}

// Levels.

/// A window with the length of `id` resetting in an hour.
fn win(id: &str, used: f64) -> Window {
    let (label, length_seconds) = match id {
        "5h" => ("5-hour", 18_000),
        _ => ("Weekly", 604_800),
    };
    window(
        id,
        label,
        length_seconds,
        used,
        Some(NOW_MS + 3_600_000),
        false,
    )
}

#[test]
fn a_window_below_75_percent_is_normal() {
    assert_eq!(level_of(&win("7d", 0.0)), UsageLevel::Normal);
    assert_eq!(level_of(&win("7d", 0.749)), UsageLevel::Normal);
}

#[test]
fn a_window_from_75_percent_is_high() {
    assert_eq!(level_of(&win("7d", 0.75)), UsageLevel::High);
    assert_eq!(level_of(&win("7d", 0.899)), UsageLevel::High);
}

#[test]
fn a_window_from_90_percent_is_critical() {
    assert_eq!(level_of(&win("7d", 0.9)), UsageLevel::Critical);
    assert_eq!(level_of(&win("7d", 0.999)), UsageLevel::Critical);
}

#[test]
fn a_window_at_100_percent_or_more_is_at_its_limit() {
    assert_eq!(level_of(&win("7d", 1.0)), UsageLevel::Limit);
    assert_eq!(level_of(&win("5h", 1.2)), UsageLevel::Limit);
}

#[test]
fn a_window_the_provider_flags_as_at_its_limit_is_at_its_limit() {
    let flagged = Window {
        at_limit: true,
        ..win("5h", 0.5)
    };
    assert_eq!(level_of(&flagged), UsageLevel::Limit);
}
