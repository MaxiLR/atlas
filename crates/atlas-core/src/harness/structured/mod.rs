//! Tasks whose output is data (ladder, question, index, deep-dive proposals):
//! structured output against a JSON schema, or the schema in the prompt when
//! the route refuses it, with one repair request.

use crate::provider::ProviderError;

/// Why a structured task ended without a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StructuredError {
    /// The answer failed the schema or the domain check, repair included.
    #[error("the answer is invalid: {}", reasons.join("; "))]
    Invalid { reasons: Vec<String> },
    #[error(transparent)]
    Provider(#[from] ProviderError),
}
