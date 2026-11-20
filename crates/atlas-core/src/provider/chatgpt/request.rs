//! The Responses call of the Codex backend: its body, headers and endpoints.
//!
//! The body carries only the fields Atlas needs: the prompt in
//! `instructions`, the context as one user message, `store: false` and
//! `stream: true`, and web search or structured output when asked. The
//! headers follow the OMP Codex provider over SSE: the bearer token, the
//! ChatGPT account the token belongs to, the workspace's data residency when
//! it is pinned to a region, and the client's originator, version and agent.
//! Atlas names itself rather than posing as another client.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde_json::{Map, Value, json};
use url::Url;

use crate::auth::{AccessToken, token_claims};
use crate::provider::ProviderRequest;

/// The client Atlas names in `originator`, on the authorisation page too.
pub const ORIGINATOR: &str = "atlas";
/// The Codex client version Atlas speaks for: the backend gates the models
/// it lists and serves by it.
pub const CODEX_CLIENT_VERSION: &str = "0.169.0";
/// Atlas's own user agent.
pub const USER_AGENT_VALUE: &str = concat!("atlas/", env!("CARGO_PKG_VERSION"));

/// The claim of OpenAI's tokens that carries the ChatGPT account.
const AUTH_CLAIM: &str = "https://api.openai.com/auth";
/// A residency claim of an account that is not pinned to a region.
const UNCONSTRAINED_RESIDENCY: &str = "no_constraint";

const ACCOUNT_ID: &str = "chatgpt-account-id";
const RESIDENCY: &str = "x-openai-internal-codex-residency";
const ORIGINATOR_HEADER: &str = "originator";
const VERSION: &str = "version";
/// The model a Responses request routes to, as `model=<slug>`.
pub const ROUTING_HINT: &str = "x-codex-routing-hint";

/// The JSON body of `POST {api}/responses` for `request`, on `default_model`
/// when the request names no model.
pub fn build_responses_body(request: &ProviderRequest, default_model: &str) -> Value {
    let mut body = Map::new();
    body.insert(
        "model".to_owned(),
        json!(request.model.as_deref().unwrap_or(default_model)),
    );
    body.insert("instructions".to_owned(), json!(request.instructions));
    body.insert(
        "input".to_owned(),
        json!([{ "type": "message", "role": "user", "content": request.input }]),
    );
    body.insert("store".to_owned(), json!(false));
    body.insert("stream".to_owned(), json!(true));
    if request.web_search {
        body.insert("tools".to_owned(), json!([{ "type": "web_search" }]));
    }
    if let Some(schema) = &request.schema {
        body.insert(
            "text".to_owned(),
            json!({
                "format": {
                    "type": "json_schema",
                    "name": schema.name,
                    "schema": schema.schema,
                }
            }),
        );
    }
    Value::Object(body)
}

/// The value of the routing hint for `model`.
pub fn routing_hint(model: &str) -> String {
    format!("model={model}")
}

/// The headers of every call with `token`: the bearer token, the JSON
/// content type, the account and residency the token carries, and the
/// client's originator, version and agent. A token that cannot be a header
/// value is left out, so the provider answers as for a missing login.
pub fn request_headers(token: &AccessToken) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(mut authorization) = HeaderValue::from_str(&format!("Bearer {}", token.secret())) {
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let auth = token_claims(token.secret())
        .and_then(|mut claims| claims.remove(AUTH_CLAIM))
        .unwrap_or_default();
    if let Some(account) = claim(&auth, "chatgpt_account_id") {
        headers.insert(ACCOUNT_ID, account);
    }
    if let Some(residency) = residency(&auth) {
        headers.insert(RESIDENCY, residency);
    }
    headers.insert(ORIGINATOR_HEADER, HeaderValue::from_static(ORIGINATOR));
    headers.insert(VERSION, HeaderValue::from_static(CODEX_CLIENT_VERSION));
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers
}

/// A non-blank string claim of the auth claim, as a header value.
fn claim(auth: &Value, name: &str) -> Option<HeaderValue> {
    let value = auth.get(name)?.as_str()?.trim();
    if value.is_empty() {
        return None;
    }
    HeaderValue::from_str(value).ok()
}

/// The workspace's residency: the data residency, else the compute
/// residency; the first one present decides, and an unconstrained one
/// declares none.
fn residency(auth: &Value) -> Option<HeaderValue> {
    let first = ["chatgpt_data_residency", "chatgpt_compute_residency"]
        .into_iter()
        .find_map(|name| {
            let value = auth.get(name)?.as_str()?.trim();
            (!value.is_empty()).then_some(value)
        })?;
    if first.eq_ignore_ascii_case(UNCONSTRAINED_RESIDENCY) {
        return None;
    }
    HeaderValue::from_str(first).ok()
}

/// `{api}/responses`, whether or not the base ends in a slash.
pub fn responses_url(api: &Url) -> Url {
    under(api, "responses")
}

/// `{api}/models?client_version=…`, whether or not the base ends in a slash.
pub fn models_url(api: &Url) -> Url {
    let mut url = under(api, "models");
    url.query_pairs_mut()
        .append_pair("client_version", CODEX_CLIENT_VERSION);
    url
}

fn under(api: &Url, segment: &str) -> Url {
    let mut url = api.clone();
    url.path_segments_mut()
        .expect("the API base is a hierarchical URL")
        .pop_if_empty()
        .push(segment);
    url
}
