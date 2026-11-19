//! The attempts of one call to the plan route: each sent with the current
//! bearer token, a refused token refreshed once, temporary failures retried
//! up to three times with growing pauses.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::{Client, Request, Response};

use super::errors::{self, Failure};
use super::request_headers;
use crate::auth::AccessToken;
use crate::provider::{ProviderError, TokenSource};

/// Retries after the first attempt.
const MAX_RETRIES: u32 = 3;

/// The header carrying the id the plan route gives each request.
const REQUEST_ID: &str = "x-request-id";

/// The request id of an answer, for diagnostics.
pub(super) fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// The pause before retry `retry` (1-based): 1 s, 2 s, 4 s.
fn pause(retry: u32) -> Duration {
    Duration::from_secs(1 << (retry - 1))
}

pub(super) struct Attempts {
    client: Client,
    tokens: Arc<dyn TokenSource>,
    /// Attempts sent so far.
    made: u32,
    retries: u32,
    refreshed: bool,
    /// The token a refresh gave, for the next attempt.
    fresh: Option<AccessToken>,
}

impl Attempts {
    pub(super) fn new(client: Client, tokens: Arc<dyn TokenSource>) -> Self {
        Self {
            client,
            tokens,
            made: 0,
            retries: 0,
            refreshed: false,
            fresh: None,
        }
    }

    /// Attempts sent so far.
    pub(super) fn made(&self) -> u32 {
        self.made
    }

    /// Sends `request` once with the bearer token: the answer when it
    /// succeeds, otherwise what the failure means.
    pub(super) async fn send(&mut self, request: &Request) -> Result<Response, Failure> {
        let token = match self.fresh.take() {
            Some(token) => token,
            None => self.tokens.access_token().await.map_err(Failure::Final)?,
        };
        let mut request = request
            .try_clone()
            .expect("the request body is held in memory");
        request.headers_mut().extend(request_headers(&token));
        self.made += 1;
        let response = self
            .client
            .execute(request)
            .await
            .map_err(|error| Failure::Temporary {
                unreachable: error.is_connect(),
                request_id: None,
            })?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status().as_u16();
        let request_id = request_id(response.headers());
        let body = response.bytes().await.unwrap_or_default();
        Err(errors::refusal(status, &body, request_id))
    }

    /// After a failed attempt: the pause before the next one (none for a
    /// repeat with a refreshed token), or the error the call ends with.
    pub(super) async fn recover(
        &mut self,
        failure: Failure,
    ) -> Result<Option<Duration>, ProviderError> {
        match failure {
            Failure::Final(error) => Err(error),
            Failure::Unauthorized { .. } if !self.refreshed => {
                self.refreshed = true;
                self.fresh = Some(self.tokens.refresh().await?);
                Ok(None)
            }
            Failure::Unauthorized { request_id } => {
                self.tokens.rejected().await;
                Err(ProviderError::SignInRequired { request_id })
            }
            Failure::NotAllowed { request_id } => {
                self.tokens.plan_not_allowed().await;
                Err(ProviderError::PlanNotAllowed { request_id })
            }
            Failure::Temporary { .. } if self.retries < MAX_RETRIES => {
                self.retries += 1;
                Ok(Some(pause(self.retries)))
            }
            Failure::Temporary {
                unreachable: true, ..
            } => Err(ProviderError::Offline),
            Failure::Temporary { request_id, .. } => Err(ProviderError::FailedAfterRetries {
                attempts: self.made,
                request_id,
            }),
        }
    }
}
