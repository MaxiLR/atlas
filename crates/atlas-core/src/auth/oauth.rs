//! The authorisation server's endpoints, its discovery document and its
//! form-encoded token requests.

use serde::Deserialize;
use serde_json::Value;
use url::Url;

/// The first-registration entry point; never saved or used for an exchange.
pub(super) const DYNAMIC_CLIENT: &str = "dynamic_agent_client";
pub(super) const AGENT_NAME: &str = "Atlas";
/// The identity scopes and the plan usage scopes, always requested together.
pub(super) const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
pub(super) const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
pub(super) const RESOURCE: &str = "https://api.openai.com/v1";

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
pub(super) fn endpoint(auth: &Url, segments: &[&str]) -> Url {
    let mut url = auth.clone();
    url.path_segments_mut()
        .expect("the authorisation server's base is a hierarchical URL")
        .pop_if_empty()
        .extend(segments);
    url
}

pub(super) fn authorize_url(auth: &Url) -> Url {
    endpoint(auth, &["api", "accounts", "authorize"])
}

pub(super) fn token_url(auth: &Url) -> Url {
    endpoint(auth, &["api", "accounts", "oauth", "token"])
}

/// The issuer the authorisation server must name: its base without a
/// trailing slash.
fn expected_issuer(auth: &Url) -> String {
    auth.as_str().trim_end_matches('/').to_owned()
}

/// What the core takes from the discovery document.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct Discovery {
    pub issuer: String,
    pub jwks_uri: Url,
    pub revocation_endpoint: Option<Url>,
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

/// The discovery document at `{auth}/.well-known/openid-configuration`; its
/// issuer must be the authorisation server itself.
pub(super) async fn discover(http: &reqwest::Client, auth: &Url) -> Result<Discovery, Failure> {
    let url = endpoint(auth, &[".well-known", "openid-configuration"]);
    let discovery: Discovery = get_json(http, url).await?;
    if discovery.issuer.trim_end_matches('/') != expected_issuer(auth) {
        return Err(Failure::Body(format!(
            "the discovery document names the issuer {}",
            discovery.issuer
        )));
    }
    Ok(discovery)
}

pub(super) async fn get_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: Url,
) -> Result<T, Failure> {
    let response = http.get(url).send().await.map_err(Failure::network)?;
    let status = response.status();
    if !status.is_success() {
        return Err(Failure::Status {
            status: status.as_u16(),
            code: None,
        });
    }
    let body = response.bytes().await.map_err(Failure::network)?;
    serde_json::from_slice(&body).map_err(|error| Failure::Body(error.to_string()))
}

/// Posts a form and returns the body of a successful answer.
pub(super) async fn post_form(
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
    /// Space-separated granted scopes.
    pub scope: Option<String>,
}

pub(super) async fn token_request(
    http: &reqwest::Client,
    auth: &Url,
    form: &[(&str, &str)],
) -> Result<TokenResponse, Failure> {
    let body = post_form(http, token_url(auth), form).await?;
    serde_json::from_slice(&body).map_err(|error| Failure::Body(error.to_string()))
}

/// The scopes in a space-separated `scope` value.
pub(super) fn scopes(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_owned).collect()
}
