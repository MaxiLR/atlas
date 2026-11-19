//! The Responses call of the plan route: its body, headers and endpoint.
//!
//! The plan route rejects `background`, `conversation`, `max_output_tokens`,
//! `max_tool_calls`, `metadata`, `moderation`, `multi_agent`, `prompt`,
//! `prompt_cache_retention`, `safety_identifier`, `temperature`,
//! `top_logprobs`, `top_p`, `truncation`, `user`, `previous_response_id` and
//! system-role input items, so the body is built from the fields it accepts
//! only. No reasoning effort is sent.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};
use url::Url;

use crate::auth::AccessToken;
use crate::provider::ProviderRequest;

/// The JSON body of `POST {api}/responses` for `request`, on `default_model`
/// when the request names no model: the prompt in `instructions`, the full
/// context as one user message in `input`, `store: false`, `stream: true`.
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

/// The bearer token and the JSON content type. A token that cannot be a header
/// value is left out, so the provider answers as for a missing login.
pub fn request_headers(token: &AccessToken) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(mut authorization) = HeaderValue::from_str(&format!("Bearer {}", token.secret())) {
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
    }
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers
}

/// `{api}/responses`, whether or not the base ends in a slash.
pub fn responses_url(api: &Url) -> Url {
    let mut url = api.clone();
    url.path_segments_mut()
        .expect("the API base is a hierarchical URL")
        .pop_if_empty()
        .push("responses");
    url
}
