//! The ChatGPT plan provider: `POST {api}/responses` with `store: false` and
//! `stream: true`, `GET {api}/models`, with the plan login's bearer token.

mod attempts;
mod errors;
mod models;
mod request;
mod stream;

pub use models::parse_models;
pub use request::{build_responses_body, models_url, request_headers, responses_url};
pub use stream::{ResponseFailure, ResponsesEvent, ResponsesStreamParser};

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, Request, Response};
use tokio::sync::Mutex;
use url::Url;

use crate::ports::{Clock, Endpoints};
use crate::protocol::Model;
use crate::provider::{
    ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, TokenSource,
};
use attempts::{Attempts, request_id};
use errors::Failure;

/// How long a connection may take to be established.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The provider for the plan route, built from the endpoints, the plan login's
/// token source and the clock that paces its retries.
///
/// A refused token is refreshed once and the request repeated; a second
/// refusal ends the login. Temporary failures are retried up to three times
/// with growing pauses. Every new attempt is announced with
/// [`ProviderEvent::Retrying`], and whatever the previous one streamed is void.
pub struct ChatGptProvider {
    client: Client,
    responses: Url,
    models: Url,
    tokens: Arc<dyn TokenSource>,
    clock: Arc<dyn Clock>,
    /// The first model listed, used when a request names none.
    default_model: Mutex<Option<String>>,
}

impl ChatGptProvider {
    pub fn new(endpoints: &Endpoints, tokens: Arc<dyn TokenSource>, clock: Arc<dyn Clock>) -> Self {
        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("the HTTP client builds");
        Self {
            client,
            responses: responses_url(&endpoints.api),
            models: models_url(&endpoints.api),
            tokens,
            clock,
            default_model: Mutex::new(None),
        }
    }

    pub fn with_idle_timeout(self, _idle: Duration) -> Self {
        self
    }

    fn attempts(&self) -> Attempts {
        Attempts::new(self.client.clone(), self.tokens.clone())
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

fn unbuildable(_: reqwest::Error) -> ProviderError {
    ProviderError::failed("the request to the plan route could not be built")
}

#[async_trait]
impl ModelProvider for ChatGptProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let body = match request.model {
            Some(_) => build_responses_body(&request, ""),
            None => build_responses_body(&request, &self.default_model().await?),
        };
        let request = self
            .client
            .post(self.responses.clone())
            .body(body.to_string())
            .build()
            .map_err(unbuildable)?;
        let exchange = Exchange {
            request,
            attempts: self.attempts(),
            clock: self.clock.clone(),
            step: Step::Open,
            pending: VecDeque::new(),
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
            match attempts.send(&request).await {
                Ok(response) => {
                    let request_id = request_id(response.headers());
                    let body = response.text().await.map_err(|_| ProviderError::Failed {
                        message: "the model list could not be read".to_owned(),
                        request_id,
                    })?;
                    return parse_models(&body);
                }
                Err(failure) => {
                    if let Some(pause) = attempts.recover(failure).await? {
                        self.clock.sleep(pause).await;
                    }
                }
            }
        }
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
}

/// One streamed response through all its attempts.
struct Exchange {
    request: Request,
    attempts: Attempts,
    clock: Arc<dyn Clock>,
    step: Step,
    /// Items ready for the caller, in order.
    pending: VecDeque<Result<ProviderEvent, ProviderError>>,
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
                        let request_id = request_id(response.headers());
                        self.step = Step::Read(Box::new(Reading {
                            response,
                            parser: ResponsesStreamParser::new(request_id.clone()),
                            request_id,
                        }));
                    }
                    Err(failure) => self.recover(failure).await,
                },
                Step::Read(mut reading) => {
                    // A connection lost mid-stream ends it like a stream
                    // without a terminal event: broken.
                    let (events, open) = match reading.response.chunk().await {
                        Ok(Some(chunk)) => (reading.parser.feed(&chunk), true),
                        Ok(None) | Err(_) => (std::mem::take(&mut reading.parser).finish(), false),
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
