//! The ChatGPT plan provider: `POST {api}/responses` with `store: false` and
//! `stream: true`, `GET {api}/models`, with the plan login's bearer token.

mod models;
mod request;
mod stream;

pub use models::parse_models;
pub use request::{build_responses_body, request_headers, responses_url};
pub use stream::{ResponseFailure, ResponsesEvent, ResponsesStreamParser};

use std::sync::Arc;

use async_trait::async_trait;

use crate::ports::{Clock, Endpoints};
use crate::protocol::Model;
use crate::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderStream, TokenSource};

/// The provider for the plan route, built from the endpoints, the plan login's
/// token source and the clock that paces its retries.
pub struct ChatGptProvider {
    endpoints: Endpoints,
    tokens: Arc<dyn TokenSource>,
    clock: Arc<dyn Clock>,
}

impl ChatGptProvider {
    pub fn new(endpoints: &Endpoints, tokens: Arc<dyn TokenSource>, clock: Arc<dyn Clock>) -> Self {
        Self {
            endpoints: endpoints.clone(),
            tokens,
            clock,
        }
    }
}

#[async_trait]
impl ModelProvider for ChatGptProvider {
    async fn stream(&self, _request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        Err(ProviderError::failed("the plan provider is not built yet"))
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        Err(ProviderError::failed("the plan provider is not built yet"))
    }
}
