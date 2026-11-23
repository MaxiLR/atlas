//! The ChatGPT plan provider on the Codex backend: `POST {api}/responses`
//! with `store: false` and `stream: true`, `GET {api}/models`, with the Codex
//! login's bearer token and the Codex client headers.

mod attempts;
mod errors;
mod models;
mod request;
mod stream;
mod usage;

pub use models::parse_models;
pub use request::{
    CODEX_CLIENT_VERSION, ORIGINATOR, ROUTING_HINT, USER_AGENT_VALUE, build_responses_body,
    models_url, request_headers, responses_url, routing_hint,
};
pub use stream::{ResponseFailure, ResponsesEvent, ResponsesStreamParser};

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, Request, Response};
use tokio::sync::Mutex;
use url::Url;

use crate::auth::AccessToken;
use crate::ports::{Clock, Endpoints};
use crate::protocol::Model;
use crate::provider::{
    ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, Task, TokenSource,
};
use attempts::{Attempts, request_id};
use errors::Failure;
use usage::usage_snapshot;

/// How long a connection may take to be established.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long an answer may stay silent, before its head or between two reads
/// of its body, before the attempt is given up as a network failure. Long
/// reasoning can leave a stream silent for minutes; this is the idle timeout
/// Codex uses on the same route.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

fn client(idle: Duration) -> Client {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(idle)
        .build()
        .expect("the HTTP client builds")
}

/// The provider for the plan route, built from the endpoints and the clock
/// that paces its retries. It draws its tokens from the plan login the
/// session hands it through [`ModelProvider::use_plan_login`]; until then
/// every call requires sign-in.
///
/// A refused token is refreshed once and the request repeated; a second
/// refusal ends the login. Temporary failures are retried up to three times
/// with growing pauses. Every new attempt is announced with
/// [`ProviderEvent::Retrying`], and whatever the previous one streamed is void.
pub struct ChatGptProvider {
    client: Client,
    responses: Url,
    models: Url,
    /// The plan login's token source, for every call from now on.
    tokens: RwLock<Arc<dyn TokenSource>>,
    clock: Arc<dyn Clock>,
    /// The first model listed, used when a request names none.
    default_model: Mutex<Option<String>>,
    /// Where a copy of each streamed body goes; the live command's only.
    capture: Option<Arc<Capture>>,
}

/// A tap and the count of the responses it has been given.
struct Capture {
    tap: Arc<dyn StreamTap>,
    responses: AtomicU64,
}

impl ChatGptProvider {
    /// The provider with `tokens` given up front instead of the session's.
    pub fn new(endpoints: &Endpoints, tokens: Arc<dyn TokenSource>, clock: Arc<dyn Clock>) -> Self {
        Self {
            client: client(IDLE_TIMEOUT),
            responses: responses_url(&endpoints.api),
            models: models_url(&endpoints.api),
            tokens: RwLock::new(tokens),
            clock,
            default_model: Mutex::new(None),
            capture: None,
        }
    }

    /// The provider before any plan login: every call requires sign-in until
    /// the session hands it its login.
    pub fn awaiting_login(endpoints: &Endpoints, clock: Arc<dyn Clock>) -> Self {
        Self::new(endpoints, Arc::new(NoLogin), clock)
    }

    /// The same provider giving up on an answer silent for `idle` instead of
    /// five minutes.
    pub fn with_idle_timeout(self, idle: Duration) -> Self {
        Self {
            client: client(idle),
            ..self
        }
    }

    /// The same provider passing `tap` a copy of each chunk of every streamed
    /// response body it reads.
    pub fn with_capture(self, tap: Arc<dyn StreamTap>) -> Self {
        Self {
            capture: Some(Arc::new(Capture {
                tap,
                responses: AtomicU64::new(0),
            })),
            ..self
        }
    }

    fn attempts(&self) -> Attempts {
        let tokens = self.tokens.read().expect("token source lock poisoned");
        Attempts::new(self.client.clone(), tokens.clone())
    }

    async fn default_model(&self) -> Result<String, ProviderError> {
        let mut default = self.default_model.lock().await;
        if let Some(model) = &*default {
            return Ok(model.clone());
        }
        let first = self
            .list_models()
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::failed("the plan lists no model"))?
            .slug;
        *default = Some(first.clone());
        Ok(first)
    }
}

/// Receives a copy of the body of each streamed response, chunk by chunk, as
/// the provider reads it: the bytes the plan route sent and nothing else,
/// never the request, its headers or a token. Set only by the live command,
/// to keep real streams as fixture candidates.
pub trait StreamTap: Send + Sync {
    /// `response` numbers the streamed responses from 1 over the provider's
    /// life, a retried attempt counting as a response of its own; `task` is
    /// the request's.
    fn chunk(&self, response: u64, task: Task, bytes: &[u8]);
}

/// The token source before a plan login is adopted.
struct NoLogin;

#[async_trait]
impl TokenSource for NoLogin {
    async fn access_token(&self) -> Result<AccessToken, ProviderError> {
        Err(ProviderError::SignInRequired { request_id: None })
    }

    async fn refresh(&self) -> Result<AccessToken, ProviderError> {
        Err(ProviderError::SignInRequired { request_id: None })
    }

    async fn rejected(&self) {}
}

fn unbuildable(_: reqwest::Error) -> ProviderError {
    ProviderError::failed("the request to the plan route could not be built")
}

#[async_trait]
impl ModelProvider for ChatGptProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let task = request.task;
        let model = match &request.model {
            Some(model) => model.clone(),
            None => self.default_model().await?,
        };
        let body = build_responses_body(&request, &model);
        let request = self
            .client
            .post(self.responses.clone())
            .header(ROUTING_HINT, routing_hint(&model))
            .body(body.to_string())
            .build()
            .map_err(unbuildable)?;
        let exchange = Exchange {
            request,
            attempts: self.attempts(),
            clock: self.clock.clone(),
            step: Step::Open,
            pending: VecDeque::new(),
            capture: self.capture.clone().map(|capture| (capture, task)),
        };
        Ok(Box::pin(futures::stream::unfold(exchange, Exchange::next)))
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        let request = self
            .client
            .get(self.models.clone())
            .build()
            .map_err(unbuildable)?;
        let mut attempts = self.attempts();
        loop {
            let failure = match attempts.send(&request).await {
                Ok(response) => {
                    let request_id = request_id(response.headers());
                    match response.text().await {
                        Ok(body) => return parse_models(&body),
                        Err(_) => Failure::interrupted(request_id),
                    }
                }
                Err(failure) => failure,
            };
            if let Some(pause) = attempts.recover(failure).await? {
                self.clock.sleep(pause).await;
            }
        }
    }

    /// Draws every later call's tokens from the plan login; this provider
    /// spends plan usage.
    fn use_plan_login(&self, tokens: Arc<dyn TokenSource>) -> bool {
        *self.tokens.write().expect("token source lock poisoned") = tokens;
        true
    }
}

/// Where a streamed response stands.
enum Step {
    /// The next attempt is sent.
    Open,
    /// The next attempt is sent after this pause.
    Pause(Duration),
    /// The attempt's stream is being read.
    Read(Box<Reading>),
    Done,
}

/// An attempt whose stream opened.
struct Reading {
    response: Response,
    parser: ResponsesStreamParser,
    request_id: Option<String>,
    /// The response's number for the capture, when there is one.
    captured: Option<u64>,
}

/// One streamed response through all its attempts.
struct Exchange {
    request: Request,
    attempts: Attempts,
    clock: Arc<dyn Clock>,
    step: Step,
    /// Items ready for the caller, in order.
    pending: VecDeque<Result<ProviderEvent, ProviderError>>,
    /// The capture, with the request's task.
    capture: Option<(Arc<Capture>, Task)>,
}

impl Exchange {
    async fn next(mut self) -> Option<(Result<ProviderEvent, ProviderError>, Self)> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                return Some((item, self));
            }
            match std::mem::replace(&mut self.step, Step::Done) {
                Step::Done => return None,
                Step::Pause(pause) => {
                    self.clock.sleep(pause).await;
                    self.step = Step::Open;
                }
                Step::Open => match self.attempts.send(&self.request).await {
                    Ok(response) => {
                        if let Some(snapshot) = usage_snapshot(response.headers()) {
                            self.deliver(ProviderEvent::Usage(snapshot));
                        }
                        let request_id = request_id(response.headers());
                        let captured = self.capture.as_ref().map(|(capture, _)| {
                            capture.responses.fetch_add(1, Ordering::Relaxed) + 1
                        });
                        self.step = Step::Read(Box::new(Reading {
                            response,
                            parser: ResponsesStreamParser::new(request_id.clone()),
                            request_id,
                            captured,
                        }));
                    }
                    Err(failure) => self.recover(failure).await,
                },
                Step::Read(mut reading) => {
                    let (events, open) = match reading.response.chunk().await {
                        Ok(Some(chunk)) => {
                            if let (Some((capture, task)), Some(number)) =
                                (&self.capture, reading.captured)
                            {
                                capture.tap.chunk(number, *task, &chunk);
                            }
                            (reading.parser.feed(&chunk), true)
                        }
                        // Without a terminal event, the ended stream is broken.
                        Ok(None) => (std::mem::take(&mut reading.parser).finish(), false),
                        Err(_) => {
                            self.recover(Failure::interrupted(reading.request_id)).await;
                            continue;
                        }
                    };
                    let mut ended = false;
                    for event in events {
                        ended |= self.receive(event, &reading.request_id).await;
                    }
                    if open && !ended {
                        self.step = Step::Read(reading);
                    }
                }
            }
        }
    }

    /// Passes one parsed event on; true when it ends the attempt.
    async fn receive(&mut self, event: ResponsesEvent, request_id: &Option<String>) -> bool {
        match event {
            ResponsesEvent::TextDelta(text) => self.deliver(ProviderEvent::TextDelta(text)),
            ResponsesEvent::Searching => self.deliver(ProviderEvent::Searching),
            ResponsesEvent::Citation(citation) => self.deliver(ProviderEvent::Citation(citation)),
            ResponsesEvent::Completed => {
                self.deliver(ProviderEvent::Completed);
                return true;
            }
            ResponsesEvent::Incomplete { reason } => {
                self.deliver(ProviderEvent::Incomplete { reason });
                return true;
            }
            ResponsesEvent::Failed(failure) => {
                self.recover(errors::response_failure(failure)).await;
                return true;
            }
            ResponsesEvent::Broken => {
                self.pending.push_back(Err(ProviderError::Incomplete {
                    request_id: request_id.clone(),
                }));
                return true;
            }
        }
        false
    }

    fn deliver(&mut self, event: ProviderEvent) {
        self.pending.push_back(Ok(event));
    }

    /// After a failed attempt: announces the next one and schedules it, or
    /// ends the stream with the error.
    async fn recover(&mut self, failure: Failure) {
        match self.attempts.recover(failure).await {
            Ok(pause) => {
                self.deliver(ProviderEvent::Retrying {
                    attempt: self.attempts.made() + 1,
                });
                self.step = pause.map_or(Step::Open, Step::Pause);
            }
            Err(error) => self.pending.push_back(Err(error)),
        }
    }
}
