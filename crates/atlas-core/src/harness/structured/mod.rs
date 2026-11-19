//! Tasks whose output is data (ladder, question, index, deep-dive proposals):
//! structured output against a JSON schema, or the schema in the prompt when
//! the route refuses it, with one repair request.

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::harness::prompts::Prompt;
use crate::provider::{ModelProvider, OutputSchema, ProviderError, Task};

/// Why a structured task ended without a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StructuredError {
    /// The answer failed the schema or the domain check, repair included.
    #[error("the answer is invalid: {}", reasons.join("; "))]
    Invalid { reasons: Vec<String> },
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// A task whose answer is data: its prompt, the JSON schema the answer must
/// follow, and the domain check the parsed value must pass.
pub trait StructuredTask: Sync {
    type Output: DeserializeOwned;

    /// The harness task its requests belong to.
    fn task(&self) -> Task;

    /// The prompt, sent as the request's instructions.
    fn prompt(&self) -> Prompt;

    /// The JSON schema of the answer.
    fn schema(&self) -> OutputSchema;

    /// What is wrong with a value that follows the schema, if anything.
    fn check(&self, value: &Self::Output) -> Result<(), Vec<String>>;
}

/// Runs structured tasks for one session. A route that refuses structured
/// output is remembered, so later tasks put the schema in the prompt at once.
pub struct StructuredRunner {
    provider: Arc<dyn ModelProvider>,
}

impl StructuredRunner {
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        Self { provider }
    }

    /// Runs `task` on `context` (the request's input), on `model` or the
    /// provider's default, and returns the checked value.
    pub async fn run<T: StructuredTask>(
        &self,
        task: &T,
        model: Option<&str>,
        context: &str,
    ) -> Result<T::Output, StructuredError> {
        let _ = (&self.provider, task, model, context);
        Err(StructuredError::Invalid {
            reasons: vec!["the structured runner is not built yet".to_owned()],
        })
    }
}
