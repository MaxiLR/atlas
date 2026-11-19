//! The usage reader, a port of Headroom's Codex provider (ADR-0007): its own
//! device-code sign-in, refresh, and `GET {chatgpt}/backend-api/wham/usage`.
//! It shares nothing with the plan login but the secret store.
//!
//! Failures are kinds only ([`UsageFailure`]): no failure carries a response
//! body or a token.

mod json;
mod login;
mod reading;

pub use reading::level_of;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use url::Url;

use crate::auth::{AccessToken, RefreshToken};
use crate::ports::{Browser, Clock, Endpoints};
use crate::protocol::UsageFailure;

/// How long a request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The device code shown while the usage login waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub user_code: String,
    /// OpenAI's page where the code is entered.
    pub verification_url: Url,
    /// When the login stops waiting for the code.
    pub expires_at: DateTime<Utc>,
}

/// Who signed in to the usage meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAccount {
    pub account_id: String,
    pub email: Option<String>,
    /// The plan as words: `Plus`, `Self serve business prolite`.
    pub plan: Option<String>,
}

/// The usage meter's login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageLogin {
    pub access_token: AccessToken,
    pub refresh_token: RefreshToken,
    pub expires_at: DateTime<Utc>,
    pub signed_in_at: DateTime<Utc>,
    pub account: UsageAccount,
}

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
    /// The time, and the pauses between polls of the login.
    pub clock: Arc<dyn Clock>,
    /// Opens OpenAI's device page.
    pub browser: Arc<dyn Browser>,
}

/// Signs in to, refreshes and reads the usage meter.
pub struct UsageReader {
    http: reqwest::Client,
    endpoints: Endpoints,
    clock: Arc<dyn Clock>,
    browser: Arc<dyn Browser>,
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
            browser: deps.browser,
        }
    }

    /// `{auth}/<segments>`.
    fn auth_url(&self, segments: &[&str]) -> Url {
        under(&self.endpoints.auth, segments)
    }

    async fn post_json(&self, url: Url, body: &Value) -> Result<reqwest::Response, UsageFailure> {
        send(self.http.post(url).json(body)).await
    }

    async fn post_form(
        &self,
        url: Url,
        fields: &[(&str, &str)],
    ) -> Result<reqwest::Response, UsageFailure> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        send(
            self.http
                .post(url)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(body),
        )
        .await
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
