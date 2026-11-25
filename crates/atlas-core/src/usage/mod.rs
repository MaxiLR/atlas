//! The usage reader, a port of Headroom's Codex provider (ADR-0007):
//! `GET {chatgpt}/backend-api/wham/usage` with the plan login's access token
//! and the headers every call of the plan route carries.
//!
//! Failures are kinds only ([`UsageFailure`]): no failure carries a response
//! body or a token.

mod json;
mod reading;

pub use reading::level_of;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use url::Url;

use crate::ports::{Clock, Endpoints};
use crate::protocol::UsageFailure;

/// How long a request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// One usage window as the account reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// `5h`, `7d`, or the length in the largest unit that divides it (`90m`);
    /// `extra:<limit>:<length>` for an additional rate limit.
    pub id: String,
    /// `5-hour`, `Weekly`, `90-minute`; `<limit> · <length>` for an
    /// additional rate limit.
    pub label: String,
    pub length_seconds: u64,
    /// The share used, from 0 to 1.
    pub used: f64,
    pub resets_at: Option<DateTime<Utc>>,
    /// The provider reports the limit reached for this window.
    pub at_limit: bool,
}

/// One reading: the general windows longest first, then those of each
/// additional rate limit.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub windows: Vec<Window>,
    /// The plan as words.
    pub plan: Option<String>,
}

/// What the reader needs from its host.
pub struct UsageReaderDeps {
    pub endpoints: Endpoints,
    /// The time the windows' resets are counted from.
    pub clock: Arc<dyn Clock>,
}

/// Reads the plan's usage.
pub struct UsageReader {
    http: reqwest::Client,
    endpoints: Endpoints,
    clock: Arc<dyn Clock>,
}

impl UsageReader {
    pub fn new(deps: UsageReaderDeps) -> Self {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("the HTTP client builds");
        Self {
            http,
            endpoints: deps.endpoints,
            clock: deps.clock,
        }
    }
}

/// `base` with `segments` appended, whether or not it ends in a slash.
fn under(base: &Url, segments: &[&str]) -> Url {
    let mut url = base.clone();
    url.path_segments_mut()
        .expect("an endpoint is a hierarchical URL")
        .pop_if_empty()
        .extend(segments);
    url
}

/// Sends `request`; no answer at all is a network failure.
async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, UsageFailure> {
    request.send().await.map_err(|_| UsageFailure::Network)
}

/// The body of a successful answer as a JSON object.
async fn object_of(response: reqwest::Response) -> Result<Map<String, Value>, UsageFailure> {
    if !response.status().is_success() {
        return Err(UsageFailure::Unexpected);
    }
    let body = response
        .bytes()
        .await
        .map_err(|_| UsageFailure::Unexpected)?;
    match serde_json::from_slice(&body) {
        Ok(Value::Object(object)) => Ok(object),
        _ => Err(UsageFailure::Unexpected),
    }
}
