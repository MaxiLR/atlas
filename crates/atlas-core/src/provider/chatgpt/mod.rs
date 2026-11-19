//! The ChatGPT plan provider: `POST {api}/responses` with `store: false` and
//! `stream: true`, `GET {api}/models`, with the plan login's bearer token.

mod models;
mod request;

pub use models::parse_models;
pub use request::{build_responses_body, request_headers, responses_url};

use std::sync::Arc;

use crate::ports::{Clock, Endpoints};
use crate::provider::TokenSource;

/// The provider for the plan route, built from the endpoints, the plan login's
/// token source and the clock that paces its retries.
pub struct ChatGptProvider {
    pub endpoints: Endpoints,
    pub tokens: Arc<dyn TokenSource>,
    pub clock: Arc<dyn Clock>,
}
