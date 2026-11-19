//! The Responses call of the plan route.

use reqwest::header::HeaderMap;
use serde_json::Value;
use url::Url;

use crate::auth::AccessToken;
use crate::provider::ProviderRequest;

pub fn build_responses_body(_request: &ProviderRequest, _default_model: &str) -> Value {
    Value::Null
}

pub fn request_headers(_token: &AccessToken) -> HeaderMap {
    HeaderMap::new()
}

pub fn responses_url(api: &Url) -> Url {
    api.clone()
}
