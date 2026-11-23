//! The ChatGPT plan provider on the Codex backend against a scripted server
//! on 127.0.0.1, with a fake clock and a fake token source: what each Codex
//! refusal becomes, how many requests it takes, and which pauses it asks of
//! the clock.

mod support;

use std::sync::Arc;
use std::time::Duration;

use atlas_core::ports::Endpoints;
use atlas_core::protocol::{CoreError, Model};
use atlas_core::provider::{
    ModelProvider, ProviderChoice, ProviderError, ProviderEvent, ProviderRequest, Task,
    UsageSnapshot, WindowUsage, build, complete_text,
};
use atlas_core::testing::{FakeClock, FakeTokenSource};
use futures::StreamExt;
use serde_json::{Value, json};
use support::server::{RecordedRequest, Reply, Route, TestServer};
use url::Url;

/// `std`'s `assert_eq!`, except that an empty array literal on the right takes
/// the element type of the left. The core links `time` (through the ID token
/// check's `jsonwebtoken`), which adds a second `PartialEq` for
/// `std::time::Duration`, so a bare `[]` no longer infers its element type.
macro_rules! assert_eq {
    ($left:expr, [] $(, $($message:tt)+)?) => {
        match &$left {
            left => ::core::assert_eq!(*left, empty_like(left) $(, $($message)+)?),
        }
    };
    ($($arguments:tt)+) => {
        ::core::assert_eq!($($arguments)+)
    };
}

/// An empty array of the slice's element type.
fn empty_like<T>(_: &[T]) -> [T; 0] {
    []
}

const TOKEN: &str = "tok-5d1e-SECRET-a91f";
const REQUEST_ID: &str = "req_7c2b9e41d0";
const RESPONSES: &str = "/api/responses";
const MODELS: &str = "/api/models";
const MODEL: &str = "gpt-test";

/// The pauses of three retries, in order.
const BACKOFF: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

fn fixture(path: &str) -> String {
    let path = format!("{}/tests/fixtures/{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn error_body(name: &str) -> String {
    fixture(&format!("errors/{name}"))
}

/// The human-readable text a body carries: `detail` or `error.message`.
fn body_message(body: &str) -> String {
    let value: Value = serde_json::from_str(body).expect("a JSON fixture");
    value
        .get("detail")
        .or_else(|| value.pointer("/error/message"))
        .and_then(Value::as_str)
        .expect("a message in the fixture")
        .to_owned()
}

fn rid() -> Option<String> {
    Some(REQUEST_ID.to_owned())
}

/// A refusal before any stream: `status` with the fixture's body.
fn refusal(status: u16, name: &str) -> Reply {
    Reply::new(status, "application/json", error_body(name)).header("x-request-id", REQUEST_ID)
}

/// A stream that writes `Hello, world!` and completes.
fn completed() -> Reply {
    Reply::sse(&fixture("responses/completed.sse")).header("x-request-id", REQUEST_ID)
}

/// A stream that writes `Partial text`, then fails with the fixture's error
/// object as the `error` of `response.failed`.
fn failing_stream(name: &str) -> Reply {
    let body: Value = serde_json::from_str(&error_body(name)).expect("a JSON fixture");
    let error = body.get("error").expect("an error object").clone();
    let events = [
        json!({"type": "response.created", "sequence_number": 0,
               "response": {"id": "resp_1", "object": "response", "status": "in_progress"}}),
        json!({"type": "response.output_text.delta", "sequence_number": 1, "item_id": "msg_1",
               "output_index": 0, "content_index": 0, "delta": "Partial "}),
        json!({"type": "response.output_text.delta", "sequence_number": 2, "item_id": "msg_1",
               "output_index": 0, "content_index": 0, "delta": "text"}),
        json!({"type": "response.failed", "sequence_number": 3,
               "response": {"id": "resp_1", "object": "response", "status": "failed",
                            "error": error}}),
    ];
    let mut sse = String::new();
    for event in events {
        let kind = event["type"].as_str().expect("a type").to_owned();
        sse.push_str(&format!("event: {kind}\ndata: {event}\n\n"));
    }
    Reply::sse(&sse).header("x-request-id", REQUEST_ID)
}

fn partial_text() -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::TextDelta("Partial ".to_owned()),
        ProviderEvent::TextDelta("text".to_owned()),
    ]
}

fn hello_world() -> Vec<ProviderEvent> {
    vec![
        ProviderEvent::TextDelta("Hello".to_owned()),
        ProviderEvent::TextDelta(", world".to_owned()),
        ProviderEvent::TextDelta("!".to_owned()),
        ProviderEvent::Completed,
    ]
}

fn request() -> ProviderRequest {
    ProviderRequest {
        model: Some(MODEL.to_owned()),
        ..ProviderRequest::new(Task::SideChatReply, "Answer the question.", "Why?")
    }
}

/// What one request came to: the events delivered, then the error that ended
/// it, whether `stream` returned it or the stream yielded it.
#[derive(Debug)]
struct Run {
    events: Vec<ProviderEvent>,
    error: Option<ProviderError>,
}

struct Rig {
    server: TestServer,
    clock: Arc<FakeClock>,
    tokens: Arc<FakeTokenSource>,
    provider: Arc<dyn ModelProvider>,
}

impl Rig {
    /// `POST /api/responses` answers `replies` in order, the last repeating.
    async fn responses(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self::routes([Route::post(RESPONSES, replies)]).await
    }

    async fn routes(routes: impl IntoIterator<Item = Route>) -> Self {
        let server = TestServer::start(routes).await;
        let endpoints = server.endpoints();
        Self::on(server, endpoints)
    }

    fn on(server: TestServer, endpoints: Endpoints) -> Self {
        let clock = Arc::new(FakeClock::default());
        let tokens = Arc::new(FakeTokenSource::new(TOKEN));
        let provider = build(
            ProviderChoice::Plan,
            &endpoints,
            tokens.clone(),
            clock.clone(),
        );
        Self {
            server,
            clock,
            tokens,
            provider,
        }
    }

    async fn run(&self, request: ProviderRequest) -> Run {
        let mut events = Vec::new();
        let mut stream = match self.provider.stream(request).await {
            Ok(stream) => stream,
            Err(error) => {
                return Run {
                    events,
                    error: Some(error),
                };
            }
        };
        while let Some(item) = stream.next().await {
            match item {
                Ok(event) => events.push(event),
                Err(error) => {
                    return Run {
                        events,
                        error: Some(error),
                    };
                }
            }
        }
        Run {
            events,
            error: None,
        }
    }

    fn requests_to(&self, path: &str) -> Vec<RecordedRequest> {
        self.server
            .requests()
            .into_iter()
            .filter(|request| request.path == path)
            .collect()
    }

    fn posts(&self) -> usize {
        self.requests_to(RESPONSES).len()
    }

    fn bearers(&self, path: &str) -> Vec<String> {
        self.requests_to(path)
            .iter()
            .map(|request| request.header("authorization").unwrap_or("").to_owned())
            .collect()
    }

    fn pauses(&self) -> Vec<Duration> {
        self.clock.sleeps()
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn retrying(attempts: impl IntoIterator<Item = u32>) -> Vec<ProviderEvent> {
    attempts
        .into_iter()
        .map(|attempt| ProviderEvent::Retrying { attempt })
        .collect()
}

fn retry_attempts(events: &[ProviderEvent]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|event| match event {
            ProviderEvent::Retrying { attempt } => Some(*attempt),
            _ => None,
        })
        .collect()
}

fn is_failed(error: &Option<ProviderError>) -> bool {
    matches!(error, Some(ProviderError::Failed { .. }))
}

/// Exactly one request, no pause, no refresh, the login kept.
fn assert_answered_once(rig: &Rig, run: &Run) {
    assert_eq!(rig.posts(), 1, "{run:?}");
    assert_eq!(rig.pauses(), [], "{run:?}");
    assert_eq!(rig.tokens.refreshes(), 0, "{run:?}");
    assert!(!rig.tokens.was_rejected(), "{run:?}");
    assert_eq!(run.events, [], "{run:?}");
}

/// Four requests, the three backoff pauses, each retry announced, then
/// failed with Retry possible.
fn assert_retried_then_failed(rig: &Rig, run: &Run) {
    assert_eq!(
        run.error,
        Some(ProviderError::FailedAfterRetries {
            attempts: 4,
            request_id: rid(),
        }),
        "{run:?}"
    );
    assert_eq!(rig.posts(), 4, "{run:?}");
    assert_eq!(rig.pauses(), BACKOFF, "{run:?}");
    assert_eq!(retry_attempts(&run.events), [2, 3, 4], "{run:?}");
    assert_eq!(rig.tokens.refreshes(), 0, "{run:?}");
    assert!(!rig.tokens.was_rejected(), "{run:?}");
}

/// A refresh, a second refusal, then sign-in required with the login
/// reported rejected.
fn assert_signed_out(rig: &Rig, run: &Run) {
    assert_eq!(
        run.error,
        Some(ProviderError::SignInRequired { request_id: rid() }),
        "{run:?}"
    );
    assert_eq!(rig.posts(), 2, "{run:?}");
    assert_eq!(rig.pauses(), [], "{run:?}");
    assert_eq!(rig.tokens.refreshes(), 1, "{run:?}");
    assert!(rig.tokens.was_rejected(), "{run:?}");
    assert_eq!(
        rig.bearers(RESPONSES),
        [bearer(TOKEN), bearer(&format!("{TOKEN}-1"))],
        "the repeat carries the refreshed token"
    );
}

// Before a stream opens: bodies with `detail` and no error object.

#[tokio::test]
async fn a_401_with_detail_refreshes_once_then_requires_sign_in() {
    let rig = Rig::responses([refusal(401, "detail-401.json")]).await;
    let run = rig.run(request()).await;
    assert_signed_out(&rig, &run);
}

#[tokio::test]
async fn a_401_followed_by_a_refreshed_token_succeeds_with_two_requests() {
    let rig = Rig::responses([refusal(401, "detail-401.json"), completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    let mut expected = retrying([2]);
    expected.extend(hello_world());
    assert_eq!(run.events, expected);
    assert_eq!(rig.posts(), 2);
    assert_eq!(rig.pauses(), []);
    assert_eq!(rig.tokens.refreshes(), 1);
    assert!(!rig.tokens.was_rejected());
    assert_eq!(
        rig.bearers(RESPONSES),
        [bearer(TOKEN), bearer(&format!("{TOKEN}-1"))]
    );
}

#[tokio::test]
async fn a_403_with_detail_is_plan_not_allowed() {
    let rig = Rig::responses([refusal(403, "detail-403.json")]).await;
    let run = rig.run(request()).await;
    assert_eq!(
        run.error,
        Some(ProviderError::PlanNotAllowed { request_id: rid() })
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn a_503_with_detail_is_retried_three_times_then_failed_with_retry() {
    let rig = Rig::responses([refusal(503, "detail-503.json")]).await;
    let run = rig.run(request()).await;
    assert_retried_then_failed(&rig, &run);
    assert_eq!(run.events, retrying([2, 3, 4]), "nothing was streamed");
}

#[tokio::test]
async fn a_503_that_clears_completes_after_its_pauses() {
    let rig = Rig::responses([
        refusal(503, "detail-503.json"),
        refusal(503, "detail-503.json"),
        completed(),
    ])
    .await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    let mut expected = retrying([2, 3]);
    expected.extend(hello_world());
    assert_eq!(run.events, expected);
    assert_eq!(rig.posts(), 3);
    assert_eq!(rig.pauses(), BACKOFF[..2]);
}

// Codex errors before a stream opens.

/// When `usage-limit-exceeded.json` says the limit resets: 2026-01-01T01:00Z.
fn limit_resets_at() -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp(1_767_229_200, 0)
}

#[tokio::test]
async fn usage_not_included_is_plan_not_allowed_with_exactly_one_request() {
    let rig = Rig::responses([refusal(403, "usage-not-included.json")]).await;
    let run = rig.run(request()).await;
    assert_eq!(
        run.error,
        Some(ProviderError::PlanNotAllowed { request_id: rid() })
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn a_usage_limit_is_plan_limit_with_exactly_one_request_and_its_reset_time() {
    let rig = Rig::responses([refusal(429, "usage-limit-exceeded.json")]).await;
    let run = rig.run(request()).await;
    assert_eq!(
        run.error,
        Some(ProviderError::PlanLimit {
            resets_at: limit_resets_at(),
            request_id: rid(),
        })
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn an_overloaded_server_is_retried_three_times_then_failed_with_retry() {
    let rig = Rig::responses([refusal(503, "server-overloaded.json")]).await;
    let run = rig.run(request()).await;
    assert_retried_then_failed(&rig, &run);
}

#[tokio::test]
async fn a_rate_limit_is_retried_three_times_then_failed_with_retry() {
    let rig = Rig::responses([refusal(429, "rate-limit-exceeded.json")]).await;
    let run = rig.run(request()).await;
    assert_retried_then_failed(&rig, &run);
}

#[tokio::test]
async fn an_unsupported_parameter_is_unsupported_with_its_param() {
    let rig = Rig::responses([refusal(400, "unsupported-capability.json")]).await;
    let run = rig.run(request()).await;
    assert_eq!(
        run.error,
        Some(ProviderError::Unsupported {
            param: Some("tools".to_owned()),
            request_id: rid(),
        })
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn an_expired_token_refreshes_once_then_requires_sign_in() {
    let rig = Rig::responses([refusal(401, "token-expired.json")]).await;
    let run = rig.run(request()).await;
    assert_signed_out(&rig, &run);
}

#[tokio::test]
async fn an_expired_token_followed_by_a_refreshed_token_succeeds() {
    let rig = Rig::responses([refusal(401, "token-expired.json"), completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    assert_eq!(rig.posts(), 2);
    assert_eq!(rig.tokens.refreshes(), 1);
    assert!(!rig.tokens.was_rejected());
}

#[tokio::test]
async fn an_unknown_code_fails_without_retry() {
    let rig = Rig::responses([refusal(400, "unknown-code.json")]).await;
    let run = rig.run(request()).await;
    assert!(is_failed(&run.error), "{run:?}");
    assert_eq!(
        run.error.as_ref().and_then(ProviderError::request_id),
        Some(REQUEST_ID),
        "the request id is kept"
    );
    assert_answered_once(&rig, &run);
}

// The same errors inside a stream, after text.

/// Each coded fixture with the status the Codex backend gives it.
const CODED: [(u16, &str); 7] = [
    (403, "usage-not-included.json"),
    (429, "usage-limit-exceeded.json"),
    (503, "server-overloaded.json"),
    (429, "rate-limit-exceeded.json"),
    (400, "unsupported-capability.json"),
    (401, "token-expired.json"),
    (400, "unknown-code.json"),
];

#[tokio::test]
async fn a_failure_inside_a_stream_after_text_maps_to_the_same_state_as_before_it() {
    for (status, name) in CODED {
        let before = Rig::responses([refusal(status, name)]).await;
        let before_run = before.run(request()).await;
        let inside = Rig::responses([failing_stream(name)]).await;
        let inside_run = inside.run(request()).await;

        assert!(before_run.error.is_some(), "{name}: {before_run:?}");
        if is_failed(&before_run.error) {
            assert!(is_failed(&inside_run.error), "{name}: {inside_run:?}");
        } else {
            assert_eq!(inside_run.error, before_run.error, "{name}");
        }
        assert_eq!(
            inside_run
                .error
                .as_ref()
                .and_then(ProviderError::request_id),
            Some(REQUEST_ID),
            "{name}: the request id is kept"
        );
        assert_eq!(inside.posts(), before.posts(), "{name}: requests");
        assert_eq!(inside.pauses(), before.pauses(), "{name}: pauses");
        assert_eq!(
            inside.tokens.refreshes(),
            before.tokens.refreshes(),
            "{name}: refreshes"
        );
        assert_eq!(
            inside.tokens.was_rejected(),
            before.tokens.was_rejected(),
            "{name}: rejected"
        );
        assert_eq!(
            inside_run.events[..2],
            partial_text(),
            "{name}: the text arrives before the failure"
        );
        assert_eq!(
            retry_attempts(&inside_run.events),
            retry_attempts(&before_run.events),
            "{name}: each new attempt is announced"
        );
    }
}

#[tokio::test]
async fn a_retry_after_streamed_text_says_so_and_starts_over() {
    let rig = Rig::responses([failing_stream("server-overloaded.json"), completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    let mut expected = partial_text();
    expected.extend(retrying([2]));
    expected.extend(hello_world());
    assert_eq!(run.events, expected);
    assert_eq!(rig.posts(), 2);
    assert_eq!(rig.pauses(), BACKOFF[..1]);

    let rig = Rig::responses([failing_stream("server-overloaded.json"), completed()]).await;
    let text = complete_text(rig.provider.as_ref(), request(), |_| {}).await;
    assert_eq!(text, Ok("Hello, world!".to_owned()));
}

// The connection.

/// A local address nothing listens on.
async fn closed_endpoints() -> Endpoints {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a free port");
    let address = listener.local_addr().expect("the bound address");
    drop(listener);
    Endpoints::local(&Url::parse(&format!("http://{address}/")).expect("a valid URL"))
}

#[tokio::test]
async fn a_connection_that_cannot_be_established_after_the_retries_is_offline() {
    let server = TestServer::start([]).await;
    let rig = Rig::on(server, closed_endpoints().await);
    let run = rig.run(request()).await;
    assert_eq!(run.error, Some(ProviderError::Offline), "{run:?}");
    assert_eq!(run.events, retrying([2, 3, 4]));
    assert_eq!(rig.pauses(), BACKOFF);
    assert_eq!(rig.tokens.refreshes(), 0);
    assert!(!rig.tokens.was_rejected());

    let models = rig.provider.list_models().await;
    assert_eq!(models, Err(ProviderError::Offline));
    assert_eq!(rig.pauses().len(), 6, "the listing retries too");
}

// Requests and the model list.

#[tokio::test]
async fn a_response_is_a_streaming_post_with_the_bearer_token() {
    let rig = Rig::responses([completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    assert_eq!(run.events, hello_world());
    let requests = rig.requests_to(RESPONSES);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].header("authorization"), Some(&*bearer(TOKEN)));
    let body: Value = serde_json::from_slice(&requests[0].body).expect("a JSON body");
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(rig.pauses(), []);
}

fn model_list() -> Reply {
    Reply::json(
        200,
        &json!({
            "models": [
                { "slug": "gpt-newest", "display_name": "GPT Newest", "visibility": "list" },
                { "slug": "gpt-internal", "display_name": "Internal", "visibility": "hide" },
                { "slug": "gpt-fast", "display_name": "GPT Fast", "visibility": "list" },
                { "slug": "gpt-older", "display_name": "GPT Older", "visibility": "list" }
            ]
        }),
    )
}

fn listed() -> Vec<Model> {
    [
        ("gpt-newest", "GPT Newest"),
        ("gpt-fast", "GPT Fast"),
        ("gpt-older", "GPT Older"),
    ]
    .into_iter()
    .map(|(slug, display_name)| Model {
        slug: slug.to_owned(),
        display_name: display_name.to_owned(),
    })
    .collect()
}

#[tokio::test]
async fn model_listing_returns_the_listed_models_in_order() {
    let rig = Rig::routes([Route::get(MODELS, [model_list()])]).await;
    assert_eq!(rig.provider.list_models().await, Ok(listed()));
    let requests = rig.requests_to(MODELS);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].header("authorization"), Some(&*bearer(TOKEN)));
}

#[tokio::test]
async fn model_listing_recovers_like_a_response() {
    let rig = Rig::routes([Route::get(
        MODELS,
        [
            refusal(401, "detail-401.json"),
            refusal(503, "detail-503.json"),
            model_list(),
        ],
    )])
    .await;
    assert_eq!(rig.provider.list_models().await, Ok(listed()));
    assert_eq!(rig.requests_to(MODELS).len(), 3);
    assert_eq!(rig.tokens.refreshes(), 1);
    assert_eq!(rig.pauses(), BACKOFF[..1]);

    let rig = Rig::routes([Route::get(
        MODELS,
        [refusal(429, "usage-limit-exceeded.json")],
    )])
    .await;
    assert_eq!(
        rig.provider.list_models().await,
        Err(ProviderError::PlanLimit {
            resets_at: limit_resets_at(),
            request_id: rid(),
        })
    );
    assert_eq!(rig.requests_to(MODELS).len(), 1);
}

#[tokio::test]
async fn a_request_without_a_model_uses_the_first_listed() {
    let rig = Rig::routes([
        Route::get(MODELS, [model_list()]),
        Route::post(RESPONSES, [completed()]),
    ])
    .await;
    let request = ProviderRequest::new(Task::SideChatReply, "Answer the question.", "Why?");
    let run = rig.run(request.clone()).await;
    assert_eq!(run.error, None, "{run:?}");
    let run = rig.run(request).await;
    assert_eq!(run.error, None, "{run:?}");
    let posts = rig.requests_to(RESPONSES);
    assert_eq!(posts.len(), 2);
    for post in posts {
        let body: Value = serde_json::from_slice(&post.body).expect("a JSON body");
        assert_eq!(body["model"], "gpt-newest");
    }
    assert_eq!(
        rig.requests_to(MODELS).len(),
        1,
        "the default is listed once"
    );
}

#[tokio::test]
async fn the_fake_choice_answers_without_the_network() {
    let server = TestServer::start([]).await;
    let endpoints = server.endpoints();
    let provider = build(
        ProviderChoice::Fake,
        &endpoints,
        Arc::new(FakeTokenSource::new(TOKEN)),
        Arc::new(FakeClock::default()),
    );
    let text = complete_text(
        provider.as_ref(),
        ProviderRequest::new(Task::WriteSection, "Write.", "Context."),
        |_| {},
    )
    .await;
    assert!(text.is_ok(), "{text:?}");
    assert_eq!(server.requests(), []);
}

// The plan's usage in the response headers.

/// The headers of the 5-hour window: 45.5 % used, resetting at
/// 2026-11-12 10:48 UTC.
const PRIMARY: [(&str, &str); 3] = [
    ("x-codex-primary-used-percent", "45.5"),
    ("x-codex-primary-window-minutes", "300"),
    ("x-codex-primary-reset-at", "1794480480"),
];

/// The headers of the weekly window: 12 % used, resetting at
/// 2026-11-16 15:00 UTC.
const SECONDARY: [(&str, &str); 3] = [
    ("x-codex-secondary-used-percent", "12"),
    ("x-codex-secondary-window-minutes", "10080"),
    ("x-codex-secondary-reset-at", "1794841200"),
];

fn five_hour() -> WindowUsage {
    WindowUsage {
        used: 0.455,
        window_minutes: 300,
        resets_at: chrono::DateTime::from_timestamp(1_794_480_480, 0).expect("a moment"),
    }
}

fn weekly() -> WindowUsage {
    WindowUsage {
        used: 0.12,
        window_minutes: 10_080,
        resets_at: chrono::DateTime::from_timestamp(1_794_841_200, 0).expect("a moment"),
    }
}

/// `completed()` with these headers too.
fn completed_with(headers: &[(&str, &str)]) -> Reply {
    headers.iter().fold(completed(), |reply, (name, value)| {
        reply.header(name, value)
    })
}

/// `hello_world()` after the usage `snapshot`.
fn reported_then_hello_world(snapshot: UsageSnapshot) -> Vec<ProviderEvent> {
    let mut events = vec![ProviderEvent::Usage(snapshot)];
    events.extend(hello_world());
    events
}

#[tokio::test]
async fn a_response_with_usage_headers_reports_both_windows_before_its_stream() {
    let rig = Rig::responses([completed_with(&[PRIMARY, SECONDARY].concat())]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    assert_eq!(
        run.events,
        reported_then_hello_world(UsageSnapshot {
            primary: Some(five_hour()),
            secondary: Some(weekly()),
        })
    );
}

#[tokio::test]
async fn a_response_without_usage_headers_reports_no_usage() {
    let rig = Rig::responses([completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    assert_eq!(run.events, hello_world());
}

#[tokio::test]
async fn a_window_with_an_unreadable_or_missing_header_is_left_out() {
    let replace = |headers: [(&'static str, &'static str); 3], name: &str, value| {
        headers.map(|(header, old)| (header, if header == name { value } else { old }))
    };
    let without = |headers: [(&'static str, &'static str); 3], name: &str| {
        headers
            .into_iter()
            .filter(|(header, _)| *header != name)
            .collect::<Vec<_>>()
    };
    let weekly_only = UsageSnapshot {
        primary: None,
        secondary: Some(weekly()),
    };
    let five_hour_only = UsageSnapshot {
        primary: Some(five_hour()),
        secondary: None,
    };
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Vec<(&str, &str)>, Option<UsageSnapshot>)> = vec![
        (
            "an unreadable share",
            [
                replace(PRIMARY, "x-codex-primary-used-percent", "lots").to_vec(),
                SECONDARY.to_vec(),
            ]
            .concat(),
            Some(weekly_only.clone()),
        ),
        (
            "an unreadable length",
            [
                replace(PRIMARY, "x-codex-primary-window-minutes", "five hours").to_vec(),
                SECONDARY.to_vec(),
            ]
            .concat(),
            Some(weekly_only.clone()),
        ),
        (
            "an unreadable reset time",
            [
                replace(PRIMARY, "x-codex-primary-reset-at", "soon").to_vec(),
                SECONDARY.to_vec(),
            ]
            .concat(),
            Some(weekly_only.clone()),
        ),
        (
            "a missing reset time",
            [
                without(PRIMARY, "x-codex-primary-reset-at"),
                SECONDARY.to_vec(),
            ]
            .concat(),
            Some(weekly_only),
        ),
        (
            "an unreadable weekly share",
            [
                PRIMARY.to_vec(),
                replace(SECONDARY, "x-codex-secondary-used-percent", "").to_vec(),
            ]
            .concat(),
            Some(five_hour_only),
        ),
        (
            "no readable window",
            [
                replace(PRIMARY, "x-codex-primary-used-percent", "NaN").to_vec(),
                without(SECONDARY, "x-codex-secondary-window-minutes"),
            ]
            .concat(),
            None,
        ),
    ];
    for (case, headers, snapshot) in cases {
        let rig = Rig::responses([completed_with(&headers)]).await;
        let run = rig.run(request()).await;
        assert_eq!(run.error, None, "{case}: {run:?}");
        let expected = match snapshot {
            Some(snapshot) => reported_then_hello_world(snapshot),
            None => hello_world(),
        };
        assert_eq!(run.events, expected, "{case}");
    }
}

// Diagnostics.

#[tokio::test]
async fn no_error_shows_the_token_or_the_served_body() {
    let mut cases: Vec<(u16, &str)> = vec![
        (401, "detail-401.json"),
        (403, "detail-403.json"),
        (503, "detail-503.json"),
    ];
    cases.extend(CODED);
    let secrets = [
        TOKEN.to_owned(),
        format!("{TOKEN}-1"),
        "5d1e-SECRET".to_owned(),
    ];

    for (status, name) in cases {
        let body = error_body(name);
        let message = body_message(&body);
        let mut runs = vec![(body.clone(), Rig::responses([refusal(status, name)]).await)];
        if !name.starts_with("detail-") {
            let reply = failing_stream(name);
            let served = String::from_utf8(reply.body.clone()).expect("UTF-8");
            runs.push((served, Rig::responses([reply]).await));
        }
        for (served, rig) in runs {
            let error = rig.run(request()).await.error.expect("an error");
            let core = CoreError::from(error.clone());
            let shown = [
                error.to_string(),
                format!("{error:?}"),
                format!("{error:#?}"),
                core.to_string(),
                format!("{core:?}"),
            ];
            for text in shown {
                for secret in &secrets {
                    assert!(!text.contains(secret), "{name}: the token in {text}");
                }
                assert!(!text.contains(&served), "{name}: the body in {text}");
                assert!(
                    !text.contains(&message),
                    "{name}: the body's text in {text}"
                );
            }
        }
    }
}

// A connection lost or gone silent after the stream opened.

mod connection_loss {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use atlas_core::provider::chatgpt::ChatGptProvider;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use super::*;

    /// How the raw server answers one request.
    #[derive(Debug, Clone, Copy)]
    enum Answer {
        /// The head of a stream and `Partial text`, then the connection drops.
        Drop,
        /// The head of a stream and `Partial text`, then silence on an open
        /// connection.
        Silent,
        /// No answer at all on an open connection.
        Mute,
        /// The whole `Hello, world!` stream.
        Complete,
        /// The whole model list.
        Models,
    }

    /// The bytes of one HTTP/1.1 chunk.
    fn chunk(data: &[u8]) -> Vec<u8> {
        let mut bytes = format!("{:x}\r\n", data.len()).into_bytes();
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(b"\r\n");
        bytes
    }

    fn partial_sse() -> String {
        let events = [
            json!({"type": "response.created", "sequence_number": 0,
                   "response": {"id": "resp_1", "object": "response", "status": "in_progress"}}),
            json!({"type": "response.output_text.delta", "sequence_number": 1, "item_id": "msg_1",
                   "output_index": 0, "content_index": 0, "delta": "Partial "}),
            json!({"type": "response.output_text.delta", "sequence_number": 2, "item_id": "msg_1",
                   "output_index": 0, "content_index": 0, "delta": "text"}),
        ];
        let mut sse = String::new();
        for event in events {
            let kind = event["type"].as_str().expect("a type").to_owned();
            sse.push_str(&format!("event: {kind}\ndata: {event}\n\n"));
        }
        sse
    }

    /// Reads one request: its head, then as many body bytes as it declares.
    async fn read_request(socket: &mut TcpStream) {
        let mut received = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = socket.read(&mut buffer).await.expect("read the request");
            assert!(read > 0, "the request ended early");
            received.extend_from_slice(&buffer[..read]);
            let Some(end) = received.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let head = String::from_utf8_lossy(&received[..end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map_or(0, |value| value.trim().parse::<usize>().expect("a length"));
            if received.len() >= end + 4 + length {
                return;
            }
        }
    }

    /// A server on 127.0.0.1 answering each request with `answers` in order,
    /// the last repeating, and counting the requests it reads.
    async fn raw_server(answers: Vec<Answer>) -> (Endpoints, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let count = Arc::new(AtomicUsize::new(0));
        let served = count.clone();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.expect("accept");
                let index = served.fetch_add(1, Ordering::SeqCst);
                let answer = answers[index.min(answers.len() - 1)];
                tokio::spawn(async move {
                    read_request(&mut socket).await;
                    if let Answer::Mute = answer {
                        std::future::pending::<()>().await;
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                         x-request-id: {REQUEST_ID}\r\ntransfer-encoding: chunked\r\n\
                         connection: close\r\n\r\n"
                    );
                    let mut bytes = head.into_bytes();
                    match answer {
                        Answer::Complete => {
                            bytes.extend(chunk(fixture("responses/completed.sse").as_bytes()));
                            bytes.extend_from_slice(b"0\r\n\r\n");
                        }
                        Answer::Models => {
                            bytes.extend(chunk(&model_list().body));
                            bytes.extend_from_slice(b"0\r\n\r\n");
                        }
                        _ => bytes.extend(chunk(partial_sse().as_bytes())),
                    }
                    socket.write_all(&bytes).await.expect("write the answer");
                    socket.flush().await.expect("flush the answer");
                    if let Answer::Silent = answer {
                        std::future::pending::<()>().await;
                    }
                    // Dropping the socket cuts the stream before its last chunk.
                });
            }
        });
        let url = Url::parse(&format!("http://{address}/")).expect("a valid URL");
        (Endpoints::local(&url), count)
    }

    /// A rig on the raw server; `idle` bounds how long the provider waits
    /// for the next bytes of an answer.
    fn rig(endpoints: &Endpoints, server: TestServer, idle: Option<Duration>) -> Rig {
        let clock = Arc::new(FakeClock::default());
        let tokens = Arc::new(FakeTokenSource::new(TOKEN));
        let provider = ChatGptProvider::new(endpoints, tokens.clone(), clock.clone());
        let provider = match idle {
            Some(idle) => provider.with_idle_timeout(idle),
            None => provider,
        };
        Rig {
            server,
            clock,
            tokens,
            provider: Arc::new(provider),
        }
    }

    /// Runs `request` on `rig`, failing the test if it never ends.
    async fn run_bounded(rig: &Rig) -> Run {
        tokio::time::timeout(Duration::from_secs(20), rig.run(request()))
            .await
            .expect("the request ends instead of hanging")
    }

    const IDLE: Duration = Duration::from_millis(300);

    #[tokio::test]
    async fn a_connection_lost_mid_stream_is_retried_and_says_so() {
        let (endpoints, requests) = raw_server(vec![Answer::Drop, Answer::Complete]).await;
        let rig = rig(&endpoints, TestServer::start([]).await, None);
        let run = run_bounded(&rig).await;
        assert_eq!(run.error, None, "{run:?}");
        let mut expected = partial_text();
        expected.extend(retrying([2]));
        expected.extend(hello_world());
        assert_eq!(run.events, expected);
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert_eq!(rig.pauses(), BACKOFF[..1]);
        assert_eq!(rig.tokens.refreshes(), 0);
    }

    #[tokio::test]
    async fn a_connection_lost_mid_stream_every_time_fails_with_retry() {
        let (endpoints, requests) = raw_server(vec![Answer::Drop]).await;
        let rig = rig(&endpoints, TestServer::start([]).await, None);
        let run = run_bounded(&rig).await;
        assert_eq!(
            run.error,
            Some(ProviderError::FailedAfterRetries {
                attempts: 4,
                request_id: rid(),
            }),
            "{run:?}"
        );
        assert_eq!(retry_attempts(&run.events), [2, 3, 4], "{run:?}");
        assert_eq!(requests.load(Ordering::SeqCst), 4);
        assert_eq!(rig.pauses(), BACKOFF);
    }

    #[tokio::test]
    async fn a_stream_that_goes_silent_is_abandoned_and_retried() {
        let (endpoints, requests) = raw_server(vec![Answer::Silent, Answer::Complete]).await;
        let rig = rig(&endpoints, TestServer::start([]).await, Some(IDLE));
        let run = run_bounded(&rig).await;
        assert_eq!(run.error, None, "{run:?}");
        let mut expected = partial_text();
        expected.extend(retrying([2]));
        expected.extend(hello_world());
        assert_eq!(run.events, expected);
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert_eq!(rig.pauses(), BACKOFF[..1]);
    }

    #[tokio::test]
    async fn a_stream_that_always_goes_silent_fails_with_retry() {
        let (endpoints, requests) = raw_server(vec![Answer::Silent]).await;
        let rig = rig(&endpoints, TestServer::start([]).await, Some(IDLE));
        let run = run_bounded(&rig).await;
        assert_eq!(
            run.error,
            Some(ProviderError::FailedAfterRetries {
                attempts: 4,
                request_id: rid(),
            }),
            "{run:?}"
        );
        assert_eq!(retry_attempts(&run.events), [2, 3, 4], "{run:?}");
        assert_eq!(requests.load(Ordering::SeqCst), 4);
        assert_eq!(rig.pauses(), BACKOFF);
    }

    #[tokio::test]
    async fn a_server_that_never_answers_fails_with_retry() {
        let (endpoints, requests) = raw_server(vec![Answer::Mute]).await;
        let rig = rig(&endpoints, TestServer::start([]).await, Some(IDLE));
        let run = run_bounded(&rig).await;
        assert_eq!(
            run.error,
            Some(ProviderError::FailedAfterRetries {
                attempts: 4,
                request_id: None,
            }),
            "{run:?}"
        );
        assert_eq!(run.events, retrying([2, 3, 4]), "{run:?}");
        assert_eq!(requests.load(Ordering::SeqCst), 4);
        assert_eq!(rig.pauses(), BACKOFF);
    }

    #[tokio::test]
    async fn a_model_listing_whose_connection_drops_is_retried() {
        let (endpoints, requests) = raw_server(vec![Answer::Drop, Answer::Models]).await;
        let rig = rig(&endpoints, TestServer::start([]).await, None);
        let models = tokio::time::timeout(Duration::from_secs(20), rig.provider.list_models())
            .await
            .expect("the listing ends instead of hanging");
        assert_eq!(models, Ok(listed()));
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert_eq!(rig.pauses(), BACKOFF[..1]);
    }
}
