//! What a refusal of the plan route means and how a request recovers from it,
//! following the plan route's errors and recovery guide.
//!
//! A structured error object decides by its `code`, whether it answers the
//! request or fails the stream (`response.failed`). Before a stream opens,
//! admission may answer `{"detail": …}` without an error object; then the
//! status decides. Only codes, params, statuses and request ids are kept:
//! messages and bodies never enter an error.

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
    /// The request ends with this error.
    Final(ProviderError),
}

impl Failure {
    /// The error when nothing recovers the request: a temporary failure
    /// fails, a refused token requires sign-in.
    pub(super) fn settled(self) -> ProviderError {
        match self {
            Self::Temporary { request_id, .. } => ProviderError::Failed {
                message: "the plan route was temporarily unavailable".to_owned(),
                request_id,
            },
            Self::Unauthorized { request_id } => ProviderError::SignInRequired { request_id },
            Self::Final(error) => error,
        }
    }
}

/// An answer with a status other than success, before any stream.
pub(super) fn refusal(status: u16, body: &[u8], request_id: Option<String>) -> Failure {
    let error = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|body| body.get("error").cloned());
    let field = |name: &str| {
        error
            .as_ref()
            .and_then(|error| error.get(name))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    if let Some(code) = field("code") {
        return coded(&code, field("param"), request_id);
    }
    match status {
        401 => Failure::Unauthorized { request_id },
        403 => Failure::Final(ProviderError::PlanNotAllowed { request_id }),
        503 => Failure::Temporary {
            unreachable: false,
            request_id,
        },
        _ => Failure::Final(ProviderError::Failed {
            message: format!("the plan route answered {status}"),
            request_id,
        }),
    }
}

/// A stream that ended with `response.failed` or an `error` event.
pub(super) fn response_failure(failure: ResponseFailure) -> Failure {
    match failure.code {
        Some(code) => coded(&code, failure.param, failure.request_id),
        None => Failure::Final(ProviderError::Failed {
            message: "the response failed without an error code".to_owned(),
            request_id: failure.request_id,
        }),
    }
}

/// The recovery for an error code. `subscription_sharing_route_not_supported`,
/// `chatpass_v2_scope_not_authorized`,
/// `chatpass_v2_invalid_authorization_context` and codes without a documented
/// recovery fail without a retry.
fn coded(code: &str, param: Option<String>, request_id: Option<String>) -> Failure {
    match code {
        "subscription_sharing_user_not_eligible" => {
            Failure::Final(ProviderError::PlanNotAllowed { request_id })
        }
        "subscription_sharing_usage_limit_exceeded" => Failure::Final(ProviderError::PlanLimit {
            resets_at: None,
            request_id,
        }),
        "subscription_sharing_usage_unavailable" | "subscription_sharing_user_unavailable" => {
            Failure::Temporary {
                unreachable: false,
                request_id,
            }
        }
        "subscription_sharing_unsupported_capability" => {
            Failure::Final(ProviderError::Unsupported { param, request_id })
        }
        "subscription_sharing_invalid_user" => Failure::Unauthorized { request_id },
        _ => Failure::Final(ProviderError::Failed {
            message: format!("the plan route refused the request with {code}"),
            request_id,
        }),
    }
}
