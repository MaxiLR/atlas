//! The catalog of visuals, generated from the interface's TypeScript catalog.

use serde_json::Value;

/// Why a visual spec was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", reasons.join("; "))]
pub struct InvalidSpec {
    pub reasons: Vec<String>,
}

pub fn prompt() -> &'static str {
    ""
}

pub fn version() -> u32 {
    0
}

pub fn component_names() -> Vec<&'static str> {
    Vec::new()
}

pub fn validate(_spec: &Value) -> Result<(), InvalidSpec> {
    Ok(())
}
