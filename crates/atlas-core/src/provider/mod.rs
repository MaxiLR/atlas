//! The model provider seam (ADR-0002): one trait, a scripted fake for tests
//! and for the app until the ChatGPT plan provider lands.

mod fake;

pub use fake::{FakeProvider, FakeScript};

use async_trait::async_trait;
use futures::stream::BoxStream;

/// The harness task a request belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    BuildIndex,
    WriteSection,
}

/// One call to the model: the prompt goes in `instructions`, the full context
/// in `input`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRequest {
    pub task: Task,
    pub instructions: String,
    pub input: String,
}

/// What a provider stream yields. A response counts only after `Completed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    TextDelta(String),
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("the stream ended before the response completed")]
    Incomplete,
    #[error("{0}")]
    Failed(String),
}

pub type ProviderStream = BoxStream<'static, Result<ProviderEvent, ProviderError>>;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Starts a streamed response to `request`.
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError>;
}
