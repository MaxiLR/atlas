use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why a request failed. The plan and network states are shown as they are;
/// the rest are failures with a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreError {
    #[error("{what} not found")]
    NotFound { what: String },
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    /// The plan's limit is reached; never retried automatically.
    #[error("the plan's usage limit is reached")]
    PlanLimit { resets_at: Option<DateTime<Utc>> },
    /// Signed in without permission to use the plan.
    #[error("plan usage is off")]
    PlanUsageOff,
    /// The account or workspace may not use the plan here.
    #[error("plan usage is not allowed for this account")]
    PlanNotAllowed,
    #[error("sign in with ChatGPT first")]
    SignInRequired,
    #[error("the app is offline")]
    Offline,
    /// Usage could not be checked or the network failed, every attempt.
    #[error("the request failed after {attempts} attempts")]
    FailedAfterRetries { attempts: u32 },
    #[error("the model provider failed: {message}")]
    Provider { message: String },
    #[error("the store failed: {message}")]
    Store { message: String },
    /// The feature that answers this request is not built yet.
    #[error("{feature} is not available yet")]
    Unavailable { feature: String },
}
