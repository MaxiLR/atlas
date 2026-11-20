//! The Codex authorisation server's endpoints and its form-encoded token
//! requests.

use serde::Deserialize;
use serde_json::Value;
use url::Url;

/// The scopes the Codex client asks for, as OMP and the Codex CLI do.
pub(super) const SCOPES: &str =
    "openid profile email offline_access api.connectors.read api.connectors.invoke";

/// Refresh errors after which the refresh token is unusable.
pub(super) const TERMINAL_REFRESH_CODES: &[&str] = &[
    "invalid_grant",
    "invalid_refresh_token",
    "token_expired",
    "refresh_token_expired",
    "refresh_token_invalidated",
    "refresh_token_reused",
];

/// `{auth}/<segments>`, whether or not the base ends in a slash.
fn endpoint(auth: &Url, segments: &[&str]) -> Url {
    let mut url = auth.clone();
    url.path_segments_mut()
        .expect("the authorisation server's base is a hierarchical URL")
        .pop_if_empty()
        .extend(segments);
    url
}

pub(super) fn authorize_url(auth: &Url) -> Url {
    endpoint(auth, &["oauth", "authorize"])
}

fn token_url(auth: &Url) -> Url {
    endpoint(auth, &["oauth", "token"])
}

/// The issuer the ID token must name: the authorisation server's base
/// without a trailing slash.
pub(super) fn issuer(auth: &Url) -> String {
    auth.as_str().trim_end_matches('/').to_owned()
}

/// Why a request to the authorisation server did not succeed.
#[derive(Debug, Clone)]
pub(super) enum Failure {
    /// No answer: the network or the connection failed.
    Network(String),
    /// An answer other than success, with its OAuth error code when it has one.
    Status { status: u16, code: Option<String> },
    /// A success whose body is not what was expected.
    Body(String),
}

impl Failure {
    fn network(error: reqwest::Error) -> Self {
        Self::Network(error.to_string())
    }

    /// A network failure or a server error: worth retrying, never a reason to
    /// clear anything.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Status { status, .. } => *status >= 500,
            Self::Body(_) => false,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(message) => write!(formatter, "{message}"),
            Self::Status {
                status,
                code: Some(code),
            } => write!(formatter, "HTTP {status}: {code}"),
            Self::Status { status, code: None } => write!(formatter, "HTTP {status}"),
            Self::Body(message) => write!(formatter, "unexpected answer: {message}"),
        }
    }
}

/// Posts a form and returns the body of a successful answer.
async fn post_form(
    http: &reqwest::Client,
    url: Url,
    form: &[(&str, &str)],
) -> Result<Vec<u8>, Failure> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form)
        .finish();
    let response = http
        .post(url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .header(reqwest::header::ACCEPT, "application/json")
        .body(body)
        .send()
        .await
        .map_err(Failure::network)?;
    let status = response.status();
    let body = response.bytes().await.map_err(Failure::network)?;
    if status.is_success() {
        return Ok(body.to_vec());
    }
    let code = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|body| error_code(&body));
    Err(Failure::Status {
        status: status.as_u16(),
        code,
    })
}

/// The machine-readable error code of an error body: OAuth's `error` string,
/// or the `code` of an `error` object.
fn error_code(body: &Value) -> Option<String> {
    match body.get("error") {
        Some(Value::String(code)) => Some(code.clone()),
        Some(Value::Object(error)) => error.get("code")?.as_str().map(str::to_owned),
        _ => body.get("code")?.as_str().map(str::to_owned),
    }
}

/// The token endpoint's answer to a code exchange or a refresh.
#[derive(Deserialize)]
pub(super) struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: Option<i64>,
}

pub(super) async fn token_request(
    http: &reqwest::Client,
    auth: &Url,
    form: &[(&str, &str)],
) -> Result<TokenResponse, Failure> {
    let body = post_form(http, token_url(auth), form).await?;
    serde_json::from_slice(&body).map_err(|error| Failure::Body(error.to_string()))
}
