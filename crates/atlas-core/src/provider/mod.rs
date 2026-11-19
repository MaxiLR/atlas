//! The model provider seam (ADR-0002): one trait, the ChatGPT plan provider in
//! `chatgpt/`, and a scripted fake for tests and for local development.

pub mod chatgpt;
mod fake;

pub use fake::{FakeProvider, FakeScript};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::auth::AccessToken;
use crate::protocol::{CoreError, Model};

/// The harness task a request belongs to; each has its own prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    DraftLadder,
    PlacementQuestion,
    BuildIndex,
    WriteSection,
    ProposeDeepDives,
    ExplainDifferently,
    Define,
    SideChatReply,
    RepairStructured,
    RepairBlock,
    RepairRecord,
}

/// A JSON schema the answer must follow, sent as structured output.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSchema {
    pub name: String,
    pub schema: serde_json::Value,
}

/// One call to the model: the prompt goes in `instructions`, the full context
/// in `input`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRequest {
    pub task: Task,
    /// The model slug; the provider's default when absent.
    pub model: Option<String>,
    pub instructions: String,
    pub input: String,
    /// Turn on the hosted web search tool.
    pub web_search: bool,
    /// Ask for structured output against this schema.
    pub schema: Option<OutputSchema>,
}

impl ProviderRequest {
    /// A request without search or schema, on the default model.
    pub fn new(task: Task, instructions: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            task,
            model: None,
            instructions: instructions.into(),
            input: input.into(),
            web_search: false,
            schema: None,
        }
    }
}

/// A URL citation: a range of the whole output text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCitation {
    pub url: String,
    pub title: String,
    pub start: u32,
    pub end: u32,
}

/// What a provider stream yields. A response counts only after `Completed`;
/// a stream that ends without it is broken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    TextDelta(String),
    /// A web search is running.
    Searching,
    Citation(ProviderCitation),
    /// An attempt failed and the request starts over; earlier deltas are void.
    Retrying {
        attempt: u32,
    },
    Completed,
    /// The provider ended the response early.
    Incomplete {
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("the stream ended before the response completed")]
    Incomplete,
    #[error("the plan's usage limit is reached")]
    PlanLimit { resets_at: Option<DateTime<Utc>> },
    #[error("plan usage is not allowed for this account")]
    PlanNotAllowed,
    #[error("sign in with ChatGPT first")]
    SignInRequired,
    #[error("the network is unreachable")]
    Offline,
    #[error("the request failed after {attempts} attempts")]
    FailedAfterRetries { attempts: u32 },
    /// The route refuses a capability, such as structured output or search.
    #[error("the route does not support {param:?}")]
    Unsupported { param: Option<String> },
    #[error("{0}")]
    Failed(String),
}

impl From<ProviderError> for CoreError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::PlanLimit { resets_at } => CoreError::PlanLimit { resets_at },
            ProviderError::PlanNotAllowed => CoreError::PlanNotAllowed,
            ProviderError::SignInRequired => CoreError::SignInRequired,
            ProviderError::Offline => CoreError::Offline,
            ProviderError::FailedAfterRetries { attempts } => {
                CoreError::FailedAfterRetries { attempts }
            }
            ProviderError::Incomplete
            | ProviderError::Unsupported { .. }
            | ProviderError::Failed(_) => CoreError::Provider {
                message: error.to_string(),
            },
        }
    }
}

pub type ProviderStream = BoxStream<'static, Result<ProviderEvent, ProviderError>>;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Starts a streamed response to `request`.
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError>;

    /// The models the plan lists for display, in the order reported.
    async fn list_models(&self) -> Result<Vec<Model>, ProviderError>;
}

/// A valid access token for each provider call, from the plan login.
#[async_trait]
pub trait TokenSource: Send + Sync {
    /// The current token, refreshed first when it is about to expire.
    async fn access_token(&self) -> Result<AccessToken, ProviderError>;
    /// A fresh token after the provider refused the current one.
    async fn refresh(&self) -> Result<AccessToken, ProviderError>;
    /// The provider refused a fresh token too: the login has ended.
    async fn rejected(&self);
}

/// Runs one request to completion, passing each text delta to `on_delta`, and
/// returns the whole text. Only a stream that reports completion counts.
pub async fn complete_text(
    provider: &dyn ModelProvider,
    request: ProviderRequest,
    mut on_delta: impl FnMut(&str) + Send,
) -> Result<String, ProviderError> {
    let mut stream = provider.stream(request).await?;
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        match event? {
            ProviderEvent::TextDelta(delta) => {
                on_delta(&delta);
                text.push_str(&delta);
            }
            ProviderEvent::Completed => return Ok(text),
            ProviderEvent::Incomplete { reason } => {
                return Err(ProviderError::Failed(format!(
                    "the response is incomplete: {}",
                    reason.as_deref().unwrap_or("no reason given")
                )));
            }
            ProviderEvent::Retrying { .. } => text.clear(),
            ProviderEvent::Searching | ProviderEvent::Citation(_) => {}
        }
    }
    Err(ProviderError::Incomplete)
}
