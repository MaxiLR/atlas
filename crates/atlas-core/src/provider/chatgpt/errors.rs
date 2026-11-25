//! What a refusal of the Codex backend means and how a request recovers from
//! it, after the OMP Codex provider.
//!
//! An error object decides by its `code`, or by its `type` when it names no
//! code (the Codex backend reports its usage limit as a `type`), whether it
//! answers the request or fails the stream (`response.failed`). A body
//! without a known code is decided by the status. Only codes, params,
//! statuses, reset times and request ids are kept: messages and bodies never
//! enter an error.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::ResponseFailure;
use crate::provider::ProviderError;

/// How a failed attempt goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Failure {
    /// Temporary: tried again after a pause. `unreachable` when no
    /// connection could be established.
    Temporary {
        unreachable: bool,
        request_id: Option<String>,
    },
    /// The token was refused: refreshed once, then sign-in is required.
    Unauthorized { request_id: Option<String> },
    /// The account or workspace may not use the plan: reported to the token
    /// source, and the request ends.
    NotAllowed { request_id: Option<String> },
    /// The request ends with this error.
    Final(ProviderError),
}

impl Failure {
    /// An answer whose connection was cut or went silent after it opened: a
    /// network failure, so the attempt starts over.
    pub(super) fn interrupted(request_id: Option<String>) -> Self {
        Self::Temporary {
            unreachable: false,
            request_id,
        }
    }

    /// The error when nothing recovers the request: a temporary failure
    /// fails, a refused token requires sign-in, a refused account is not
    /// allowed.
    pub(super) fn settled(self) -> ProviderError {
        match self {
            Self::Temporary { request_id, .. } => ProviderError::Failed {
                message: "the Codex backend was temporarily unavailable".to_owned(),
                request_id,
            },
            Self::Unauthorized { request_id } => ProviderError::SignInRequired { request_id },
            Self::NotAllowed { request_id } => ProviderError::PlanNotAllowed { request_id },
            Self::Final(error) => error,
        }
    }
}

/// An error code the Codex backend documents a recovery for.
fn known(code: &str) -> bool {
    matches!(
        code,
        "usage_limit_reached"
            | "usage_not_included"
            | "rate_limit_exceeded"
            | "server_is_overloaded"
            | "slow_down"
            | "unsupported_parameter"
            | "token_expired"
    )
}

/// The machine-readable code of an error object: its `code`, or its `type`
/// when that names a Codex error the backend reports without a code.
pub(super) fn error_code(error: &Value) -> Option<String> {
    let field = |name: &str| error.get(name).and_then(Value::as_str);
    field("code")
        .or_else(|| field("type").filter(|kind| known(kind)))
        .map(str::to_owned)
}

/// When a usage limit resets: `resets_at` in seconds since the epoch.
pub(super) fn resets_at(error: &Value) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(error.get("resets_at")?.as_i64()?, 0)
}

/// An answer with a status other than success, before any stream.
pub(super) fn refusal(status: u16, body: &[u8], request_id: Option<String>) -> Failure {
    let error = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|body| body.get("error").cloned());
    let code = error.as_ref().and_then(error_code);
    if status == 401 {
        return Failure::Unauthorized { request_id };
    }
    if let (Some(error), Some(code)) = (&error, code.as_deref().filter(|code| known(code))) {
        let param = error
            .get("param")
            .and_then(Value::as_str)
            .map(str::to_owned);
        return coded(code, param, resets_at(error), request_id);
    }
    match (status, code) {
        (429 | 500 | 502 | 503 | 504, _) => Failure::Temporary {
            unreachable: false,
            request_id,
        },
        (403, None) => Failure::NotAllowed { request_id },
        (_, code) => Failure::Final(ProviderError::Failed {
            message: match code {
                Some(code) => format!("the Codex backend refused the request with {code}"),
                None => format!("the Codex backend answered {status}"),
            },
            request_id,
        }),
    }
}

/// A stream that ended with `response.failed` or an `error` event.
pub(super) fn response_failure(failure: ResponseFailure) -> Failure {
    match failure.code {
        Some(code) => coded(&code, failure.param, failure.resets_at, failure.request_id),
        None => Failure::Final(ProviderError::Failed {
            message: "the response failed without an error code".to_owned(),
            request_id: failure.request_id,
        }),
    }
}

/// The recovery for an error code; codes without a known recovery fail
/// without a retry.
fn coded(
    code: &str,
    param: Option<String>,
    resets_at: Option<DateTime<Utc>>,
    request_id: Option<String>,
) -> Failure {
    match code {
        "usage_limit_reached" => Failure::Final(ProviderError::PlanLimit {
            resets_at,
            request_id,
        }),
        "usage_not_included" => Failure::NotAllowed { request_id },
        "rate_limit_exceeded" | "server_is_overloaded" | "slow_down" => Failure::Temporary {
            unreachable: false,
            request_id,
        },
        "unsupported_parameter" => Failure::Final(ProviderError::Unsupported { param, request_id }),
        "token_expired" => Failure::Unauthorized { request_id },
        _ => Failure::Final(ProviderError::Failed {
            message: format!("the Codex backend refused the request with {code}"),
            request_id,
        }),
    }
}
