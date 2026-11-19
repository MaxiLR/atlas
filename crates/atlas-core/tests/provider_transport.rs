//! The ChatGPT plan provider against a scripted server on 127.0.0.1, with a
//! fake clock and a fake token source: what each documented refusal becomes,
//! how many requests it takes, and which pauses it asks of the clock.

mod support;

use std::sync::Arc;
use std::time::Duration;

use atlas_core::ports::Endpoints;
use atlas_core::protocol::{CoreError, Model};
use atlas_core::provider::{
    ModelProvider, ProviderChoice, ProviderError, ProviderEvent, ProviderRequest, Task, build,
    complete_text,
};
use atlas_core::testing::{FakeClock, FakeTokenSource};
use futures::StreamExt;
use serde_json::{Value, json};
use support::server::{RecordedRequest, Reply, Route, TestServer};
use url::Url;

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

// Structured codes before a stream opens.

#[tokio::test]
async fn user_not_eligible_is_plan_not_allowed_with_exactly_one_request() {
    let rig = Rig::responses([refusal(403, "user-not-eligible.json")]).await;
    let run = rig.run(request()).await;
    assert_eq!(
        run.error,
        Some(ProviderError::PlanNotAllowed { request_id: rid() })
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn usage_limit_exceeded_is_plan_limit_with_exactly_one_request_and_no_reset_time() {
    let rig = Rig::responses([refusal(429, "usage-limit-exceeded.json")]).await;
    let run = rig.run(request()).await;
    assert_eq!(
        run.error,
        Some(ProviderError::PlanLimit {
            resets_at: None,
            request_id: rid(),
        })
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn usage_unavailable_is_retried_three_times_then_failed_with_retry() {
    let rig = Rig::responses([refusal(503, "usage-unavailable.json")]).await;
    let run = rig.run(request()).await;
    assert_retried_then_failed(&rig, &run);
}

#[tokio::test]
async fn user_unavailable_is_retried_three_times_then_failed_with_retry() {
    let rig = Rig::responses([refusal(503, "user-unavailable.json")]).await;
    let run = rig.run(request()).await;
    assert_retried_then_failed(&rig, &run);
}

#[tokio::test]
async fn unsupported_capability_is_unsupported_with_its_param() {
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
async fn invalid_user_refreshes_once_then_requires_sign_in() {
    let rig = Rig::responses([refusal(401, "invalid-user.json")]).await;
    let run = rig.run(request()).await;
    assert_signed_out(&rig, &run);
}

#[tokio::test]
async fn invalid_user_followed_by_a_refreshed_token_succeeds() {
    let rig = Rig::responses([refusal(401, "invalid-user.json"), completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    assert_eq!(rig.posts(), 2);
    assert_eq!(rig.tokens.refreshes(), 1);
    assert!(!rig.tokens.was_rejected());
}

async fn assert_failed_without_retry(status: u16, name: &str) {
    let rig = Rig::responses([refusal(status, name)]).await;
    let run = rig.run(request()).await;
    assert!(is_failed(&run.error), "{name}: {run:?}");
    assert_eq!(
        run.error.as_ref().and_then(ProviderError::request_id),
        Some(REQUEST_ID),
        "{name}: the request id is kept"
    );
    assert_answered_once(&rig, &run);
}

#[tokio::test]
async fn route_not_supported_fails_without_retry() {
    assert_failed_without_retry(403, "route-not-supported.json").await;
}

#[tokio::test]
async fn scope_not_authorized_fails_without_retry() {
    assert_failed_without_retry(403, "scope-not-authorized.json").await;
}

#[tokio::test]
async fn invalid_authorization_context_fails_without_retry() {
    assert_failed_without_retry(403, "invalid-authorization-context.json").await;
}

#[tokio::test]
async fn an_unknown_code_fails_without_retry() {
    assert_failed_without_retry(400, "unknown-code.json").await;
}

// The same codes inside a stream, after text.

/// Each coded fixture with the status the documentation gives it.
const CODED: [(u16, &str); 10] = [
    (403, "user-not-eligible.json"),
    (429, "usage-limit-exceeded.json"),
    (503, "usage-unavailable.json"),
    (400, "unsupported-capability.json"),
    (403, "route-not-supported.json"),
    (401, "invalid-user.json"),
    (403, "scope-not-authorized.json"),
    (403, "invalid-authorization-context.json"),
    (503, "user-unavailable.json"),
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
    let rig = Rig::responses([failing_stream("usage-unavailable.json"), completed()]).await;
    let run = rig.run(request()).await;
    assert_eq!(run.error, None, "{run:?}");
    let mut expected = partial_text();
    expected.extend(retrying([2]));
    expected.extend(hello_world());
    assert_eq!(run.events, expected);
    assert_eq!(rig.posts(), 2);
    assert_eq!(rig.pauses(), BACKOFF[..1]);

    let rig = Rig::responses([failing_stream("usage-unavailable.json"), completed()]).await;
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
            resets_at: None,
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
