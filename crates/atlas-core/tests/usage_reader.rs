//! The usage reader against a scripted server on `127.0.0.1` with a fake
//! clock: every case of Headroom's `codex-login.test.mjs`,
//! `codex-read.test.mjs` and `levels.test.mjs`, with the same inputs, the same
//! expected values and its fixtures in `tests/fixtures/usage/`.

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use atlas_core::auth::{AccessToken, RefreshToken};
use atlas_core::ports::{Clock, Endpoints};
use atlas_core::protocol::{UsageFailure, UsageLevel};
use atlas_core::testing::{FakeClock, RecordingBrowser};
use atlas_core::usage::{
    DeviceCode, Usage, UsageAccount, UsageLogin, UsageReader, UsageReaderDeps, Window, level_of,
};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use support::server::{RecordedRequest, Reply, Route, TestServer};
use url::Url;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const USERCODE: &str = "/auth/api/accounts/deviceauth/usercode";
const POLL: &str = "/auth/api/accounts/deviceauth/token";
const TOKEN: &str = "/auth/oauth/token";
const USAGE: &str = "/chatgpt/backend-api/wham/usage";
const FORM: &str = "application/x-www-form-urlencoded";

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

fn access() -> String {
    jwt(&json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-1",
            "chatgpt_plan_type": "plus",
        },
        "https://api.openai.com/profile": { "email": "maxi@example.com" },
    }))
}

fn id_token() -> String {
    jwt(&json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-2",
            "chatgpt_plan_type": "pro",
        },
        "https://api.openai.com/profile": { "email": "other@example.com" },
    }))
}

fn bare_access() -> String {
    jwt(&json!({ "sub": "user-1" }))
}

fn secrets() -> Vec<String> {
    vec![
        access(),
        id_token(),
        "refresh-secret-1".to_owned(),
        "auth-code-1".to_owned(),
        "verifier-1".to_owned(),
    ]
}

fn device_code() -> Value {
    json!({ "device_auth_id": "device-1", "user_code": "ABCD-1234", "interval": "3" })
}

fn approved() -> Value {
    json!({ "authorization_code": "auth-code-1", "code_verifier": "verifier-1" })
}

fn tokens() -> Value {
    json!({
        "access_token": access(),
        "refresh_token": "refresh-secret-1",
        "id_token": id_token(),
        "expires_in": 3600,
    })
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

/// What OpenAI answers during a sign-in: the device code, the polls in turn
/// (the last one repeating) and the exchange.
struct Answers {
    device: Reply,
    polls: Vec<Reply>,
    exchange: Reply,
}

impl Default for Answers {
    fn default() -> Self {
        Self {
            device: ok(&device_code()),
            polls: vec![ok(&approved())],
            exchange: ok(&tokens()),
        }
    }
}

struct Harness {
    server: TestServer,
    clock: Arc<FakeClock>,
    browser: Arc<RecordingBrowser>,
    reader: UsageReader,
    presented: Arc<Mutex<Vec<DeviceCode>>>,
}

impl Harness {
    async fn start(routes: Vec<Route>) -> Self {
        let server = TestServer::start(routes).await;
        let endpoints = server.endpoints();
        Self::on(server, endpoints)
    }

    fn on(server: TestServer, endpoints: Endpoints) -> Self {
        let clock = Arc::new(FakeClock::new(now()));
        let browser = Arc::new(RecordingBrowser::default());
        let reader = UsageReader::new(UsageReaderDeps {
            endpoints,
            clock: clock.clone(),
            browser: browser.clone(),
        });
        Self {
            server,
            clock,
            browser,
            reader,
            presented: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn signing_in(answers: Answers) -> Self {
        Self::start(vec![
            Route::post(USERCODE, [answers.device]),
            Route::post(POLL, answers.polls),
            Route::post(TOKEN, [answers.exchange]),
        ])
        .await
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

    async fn login(&self) -> Result<UsageLogin, UsageFailure> {
        self.login_until(|| false).await
    }

    async fn login_until(
        &self,
        cancelled: impl Fn() -> bool + Send + Sync,
    ) -> Result<UsageLogin, UsageFailure> {
        let presented = self.presented.clone();
        self.reader
            .login(
                move |code| presented.lock().expect("lock").push(code),
                cancelled,
            )
            .await
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.server.requests()
    }

    fn count(&self, path: &str) -> usize {
        self.requests().iter().filter(|r| r.path == path).count()
    }

    fn url(&self, path: &str) -> Url {
        self.server
            .url()
            .join(path.trim_start_matches('/'))
            .expect("a valid URL")
    }
}

fn form_of(request: &RecordedRequest) -> Vec<(String, String)> {
    url::form_urlencoded::parse(&request.body)
        .into_owned()
        .collect()
}

fn pairs(fields: &[(&str, &str)]) -> Vec<(String, String)> {
    fields
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn json_of(request: &RecordedRequest) -> Value {
    serde_json::from_slice(&request.body).expect("a JSON body")
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

fn seconds(count: u64) -> Duration {
    Duration::from_secs(count)
}

fn account(id: &str, email: &str, plan: &str) -> UsageAccount {
    UsageAccount {
        account_id: id.to_owned(),
        email: Some(email.to_owned()),
        plan: Some(plan.to_owned()),
    }
}

// Login.

#[tokio::test]
async fn pending_polls_then_an_approval_then_the_token_exchange() {
    let harness = Harness::signing_in(Answers {
        polls: vec![
            Reply::json(403, &json!({ "error": "pending" })),
            Reply::json(404, &json!({ "error": "pending" })),
            ok(&approved()),
        ],
        ..Answers::default()
    })
    .await;
    let result = harness.login().await;

    let requests = harness.requests();
    assert_eq!(
        requests
            .iter()
            .map(|r| (r.method.as_str(), r.path.as_str()))
            .collect::<Vec<_>>(),
        [
            ("POST", USERCODE),
            ("POST", POLL),
            ("POST", POLL),
            ("POST", POLL),
            ("POST", TOKEN),
        ],
    );
    assert_eq!(json_of(&requests[0]), json!({ "client_id": CLIENT_ID }));
    assert_eq!(
        json_of(&requests[1]),
        json!({ "device_auth_id": "device-1", "user_code": "ABCD-1234" }),
    );
    let exchange = &requests[4];
    assert_eq!(exchange.header("content-type"), Some(FORM));
    assert_eq!(
        form_of(exchange),
        pairs(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", "auth-code-1"),
            ("code_verifier", "verifier-1"),
            (
                "redirect_uri",
                harness.url("/auth/deviceauth/callback").as_str()
            ),
        ]),
    );
    assert_eq!(harness.clock.sleeps(), [seconds(3), seconds(3), seconds(3)]);

    assert_eq!(
        result,
        Ok(UsageLogin {
            access_token: AccessToken::new(access()),
            refresh_token: RefreshToken::new("refresh-secret-1".to_owned()),
            expires_at: at_ms(NOW_MS + 9000 + 3_600_000),
            signed_in_at: at_ms(NOW_MS + 9000),
            account: account("acct-1", "maxi@example.com", "Plus"),
        }),
    );
}

#[tokio::test]
async fn presents_the_device_page_the_code_and_a_15_minute_deadline() {
    let harness = Harness::signing_in(Answers::default()).await;
    harness.login().await.expect("signs in");
    let device = harness.url("/auth/codex/device");
    assert_eq!(
        *harness.presented.lock().expect("lock"),
        [DeviceCode {
            user_code: "ABCD-1234".to_owned(),
            verification_url: device.clone(),
            expires_at: at_ms(NOW_MS + 900_000),
        }],
    );
    assert_eq!(harness.browser.opened(), [device]);
}

#[tokio::test]
async fn polls_every_5_seconds_when_openai_gives_no_interval() {
    let harness = Harness::signing_in(Answers {
        device: ok(&json!({ "device_auth_id": "device-1", "user_code": "ABCD-1234" })),
        polls: vec![Reply::text(403, ""), ok(&approved())],
        ..Answers::default()
    })
    .await;
    assert!(harness.login().await.is_ok());
    assert_eq!(harness.clock.sleeps(), [seconds(5), seconds(5)]);
}

#[tokio::test]
async fn a_numeric_interval_is_taken_in_seconds() {
    let harness = Harness::signing_in(Answers {
        device: ok(&with(device_code(), json!({ "interval": 7 }))),
        polls: vec![Reply::text(404, ""), ok(&approved())],
        ..Answers::default()
    })
    .await;
    harness.login().await.expect("signs in");
    assert_eq!(harness.clock.sleeps(), [seconds(7), seconds(7)]);
}

#[tokio::test]
async fn cancelling_stops_the_polling() {
    let harness = Harness::signing_in(Answers {
        polls: vec![Reply::text(403, "")],
        ..Answers::default()
    })
    .await;
    // The dialog closes once the first poll has been answered.
    let result = harness
        .login_until(|| harness.server.requests().len() >= 2)
        .await;
    assert_eq!(result, Err(UsageFailure::Cancelled));
    assert_eq!(
        harness
            .requests()
            .iter()
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>(),
        [USERCODE, POLL],
    );
}

#[tokio::test]
async fn fifteen_minutes_without_an_approval_is_a_timeout() {
    let harness = Harness::signing_in(Answers {
        device: ok(&with(device_code(), json!({ "interval": 5 }))),
        polls: vec![Reply::text(403, "")],
        ..Answers::default()
    })
    .await;
    let result = harness.login().await;
    assert_eq!(result, Err(UsageFailure::Timeout));
    // 180 sleeps of 5 s reach the deadline; the 180th is not followed by a poll.
    assert_eq!(harness.clock.sleeps().len(), 180);
    assert_eq!(harness.clock.now(), at_ms(NOW_MS + 900_000));
    assert_eq!(harness.count(POLL), 179);
    assert_eq!(harness.count(TOKEN), 0);
}

#[tokio::test]
async fn identity_from_the_id_token_when_the_access_token_has_none() {
    let harness = Harness::signing_in(Answers {
        exchange: ok(&with(tokens(), json!({ "access_token": bare_access() }))),
        ..Answers::default()
    })
    .await;
    let login = harness.login().await.expect("signs in");
    assert_eq!(login.access_token, AccessToken::new(bare_access()));
    assert_eq!(login.account, account("acct-2", "other@example.com", "Pro"));
}

#[tokio::test]
async fn no_account_id_in_either_token_is_unexpected() {
    let harness = Harness::signing_in(Answers {
        exchange: ok(&with(
            tokens(),
            json!({ "access_token": bare_access(), "id_token": bare_access() }),
        )),
        ..Answers::default()
    })
    .await;
    let failure = harness.login().await.expect_err("fails");
    assert_eq!(failure, UsageFailure::Unexpected);
    assert_no_secret(&failure, &secrets());
}

#[tokio::test]
async fn a_rejected_fetch_is_a_network_failure() {
    let harness = Harness::offline().await;
    assert_eq!(harness.login().await, Err(UsageFailure::Network));
}

#[tokio::test]
async fn an_exchange_answered_4xx_is_rejected() {
    let harness = Harness::signing_in(Answers {
        exchange: Reply::json(400, &json!({ "error": "invalid_grant auth-code-1" })),
        ..Answers::default()
    })
    .await;
    let failure = harness.login().await.expect_err("fails");
    assert_eq!(failure, UsageFailure::Rejected);
    assert_no_secret(&failure, &secrets());
}

#[tokio::test]
async fn an_exchange_answered_500_is_unexpected() {
    let harness = Harness::signing_in(Answers {
        exchange: Reply::text(500, "failed for verifier-1"),
        ..Answers::default()
    })
    .await;
    let failure = harness.login().await.expect_err("fails");
    assert_eq!(failure, UsageFailure::Unexpected);
    assert_no_secret(&failure, &secrets());
}

#[tokio::test]
async fn a_poll_answered_with_another_status_is_unexpected() {
    let harness = Harness::signing_in(Answers {
        polls: vec![Reply::text(500, "")],
        ..Answers::default()
    })
    .await;
    assert_eq!(harness.login().await, Err(UsageFailure::Unexpected));
    assert_eq!(harness.count(TOKEN), 0);
}

#[tokio::test]
async fn a_missing_field_is_unexpected() {
    for answers in [
        Answers {
            device: ok(&json!({ "user_code": "ABCD-1234" })),
            ..Answers::default()
        },
        Answers {
            polls: vec![ok(&json!({ "code_verifier": "verifier-1" }))],
            ..Answers::default()
        },
        Answers {
            exchange: ok(&with(tokens(), json!({ "refresh_token": null }))),
            ..Answers::default()
        },
        Answers {
            exchange: ok(&with(tokens(), json!({ "expires_in": null }))),
            ..Answers::default()
        },
    ] {
        let harness = Harness::signing_in(answers).await;
        let failure = harness.login().await.expect_err("fails");
        assert_eq!(failure, UsageFailure::Unexpected);
        assert_no_secret(&failure, &secrets());
    }
}

#[tokio::test]
async fn the_identitys_plan_reads_as_words() {
    let access = jwt(&json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-1",
            "chatgpt_plan_type": "self_serve_business_prolite",
        },
        "https://api.openai.com/profile": { "email": "maxi@example.com" },
    }));
    let harness = Harness::signing_in(Answers {
        exchange: ok(&with(tokens(), json!({ "access_token": access }))),
        ..Answers::default()
    })
    .await;
    let login = harness.login().await.expect("signs in");
    assert_eq!(
        login.account.plan.as_deref(),
        Some("Self serve business prolite")
    );
}

// Refresh.

fn credentials() -> UsageLogin {
    UsageLogin {
        access_token: AccessToken::new("old-access".to_owned()),
        refresh_token: RefreshToken::new("old-refresh".to_owned()),
        expires_at: at_ms(NOW_MS - 1000),
        signed_in_at: at_ms(NOW_MS - 86_400_000),
        account: account("acct-1", "maxi@example.com", "Plus"),
    }
}

async fn refresh_with(reply: Reply) -> (Harness, Result<UsageLogin, UsageFailure>) {
    let harness = Harness::start(vec![Route::post(TOKEN, [reply])]).await;
    let result = harness.reader.refresh(&credentials()).await;
    (harness, result)
}

#[tokio::test]
async fn refresh_posts_the_refresh_token_as_a_form() {
    let (harness, _) = refresh_with(ok(
        &json!({ "access_token": "new-access", "expires_in": 3600 }),
    ))
    .await;
    let requests = harness.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.path, TOKEN);
    assert_eq!(request.method, "POST");
    assert_eq!(request.header("content-type"), Some(FORM));
    assert_eq!(
        form_of(request),
        pairs(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", "old-refresh"),
            ("client_id", CLIENT_ID),
        ]),
    );
}

#[tokio::test]
async fn refresh_takes_a_rotated_refresh_token() {
    let (_, result) = refresh_with(ok(&json!({
        "access_token": "new-access",
        "refresh_token": "new-refresh",
        "expires_in": 3600,
    })))
    .await;
    assert_eq!(
        result,
        Ok(UsageLogin {
            access_token: AccessToken::new("new-access".to_owned()),
            refresh_token: RefreshToken::new("new-refresh".to_owned()),
            expires_at: at_ms(NOW_MS + 3_600_000),
            ..credentials()
        }),
    );
}

#[tokio::test]
async fn refresh_keeps_the_old_refresh_token_when_none_comes_back() {
    let (_, result) = refresh_with(ok(
        &json!({ "access_token": "new-access", "expires_in": 864_000 }),
    ))
    .await;
    assert_eq!(
        result,
        Ok(UsageLogin {
            access_token: AccessToken::new("new-access".to_owned()),
            refresh_token: RefreshToken::new("old-refresh".to_owned()),
            expires_at: at_ms(NOW_MS + 864_000_000),
            ..credentials()
        }),
    );
}

#[tokio::test]
async fn a_refused_refresh_token_ends_the_login() {
    for status in [400, 401, 403] {
        let (_, result) = refresh_with(Reply::json(
            status,
            &json!({ "error": "invalid_grant old-refresh" }),
        ))
        .await;
        assert_eq!(result, Err(UsageFailure::Auth), "{status}");
    }
}

#[tokio::test]
async fn refresh_failures_that_are_not_the_tokens() {
    let offline = Harness::offline().await;
    let offline = offline.reader.refresh(&credentials()).await;
    assert_eq!(offline, Err(UsageFailure::Network));
    let (_, broken) = refresh_with(Reply::text(500, "error for old-refresh")).await;
    assert_eq!(broken, Err(UsageFailure::Unexpected));
    let (_, empty) = refresh_with(ok(&json!({}))).await;
    assert_eq!(empty, Err(UsageFailure::Unexpected));
    for result in [offline, broken, empty] {
        assert_no_secret(&result.expect_err("fails"), &["old-refresh".to_owned()]);
    }
}

// Read.

const READ_TOKEN: &str = "access-secret-1";

fn reading_credentials() -> UsageLogin {
    UsageLogin {
        access_token: AccessToken::new(READ_TOKEN.to_owned()),
        refresh_token: RefreshToken::new("refresh-secret-1".to_owned()),
        expires_at: at_ms(NOW_MS + 3_600_000),
        signed_in_at: at_ms(NOW_MS - 3_600_000),
        account: account("acct-1", "maxi@example.com", "Plus"),
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
    let result = harness.reader.read(&reading_credentials()).await;
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

#[tokio::test]
async fn reads_the_usage_with_the_access_token_and_the_account_id() {
    let (harness, _) = read_with(ok(&fixture("five-hour-weekly"))).await;
    let requests = harness.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.path, USAGE);
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {READ_TOKEN}").as_str())
    );
    assert_eq!(request.header("chatgpt-account-id"), Some("acct-1"));
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
async fn a_401_is_an_ended_login() {
    let (_, result) = read_with(Reply::json(
        401,
        &json!({ "error": format!("bad token {READ_TOKEN}") }),
    ))
    .await;
    assert_eq!(result, Err(UsageFailure::Auth));
}

#[tokio::test]
async fn a_rejected_read_fetch_is_a_network_failure() {
    let harness = Harness::offline().await;
    let result = harness.reader.read(&reading_credentials()).await;
    assert_eq!(result, Err(UsageFailure::Network));
    assert_no_secret(&result.expect_err("fails"), &[READ_TOKEN.to_owned()]);
}

#[tokio::test]
async fn a_500_is_unexpected_and_never_carries_its_body() {
    let (_, result) = read_with(Reply::text(
        500,
        &format!("internal error for Bearer {READ_TOKEN}"),
    ))
    .await;
    assert_eq!(result, Err(UsageFailure::Unexpected));
    assert_no_secret(&result.expect_err("fails"), &[READ_TOKEN.to_owned()]);
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
