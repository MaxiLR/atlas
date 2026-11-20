//! One sign-in attempt: the Codex authorisation page, the callback and the
//! code exchange.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

use super::callback::{self, CallbackListener};
use super::oauth::{self, SCOPES};
use super::stored::StoredLogin;
use super::{AuthError, CALLBACK_PORT, CODEX_CLIENT_ID, Inner, Login, claims};
use crate::provider::chatgpt::ORIGINATOR;

/// The per-attempt secrets: state and PKCE verifier.
struct Attempt {
    state: String,
    verifier: String,
}

impl Attempt {
    fn fresh() -> Self {
        let random = || URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
        Self {
            state: random(),
            verifier: random(),
        }
    }

    /// The S256 challenge: the unpadded base64url SHA-256 of the verifier.
    fn challenge(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()))
    }
}

/// A sign-in attempt waiting for its callback. Dropping it cancels it and
/// frees the callback port.
pub struct PendingSignIn {
    inner: Arc<Inner>,
    listener: CallbackListener,
    attempt: Attempt,
}

pub(super) async fn start(inner: Arc<Inner>) -> Result<PendingSignIn, AuthError> {
    let listener = CallbackListener::bind(CALLBACK_PORT)
        .await
        .map_err(|error| AuthError::CallbackUnavailable {
            port: CALLBACK_PORT,
            reason: error.to_string(),
        })?;
    let attempt = Attempt::fresh();

    let mut url = oauth::authorize_url(&inner.endpoints.auth);
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", CODEX_CLIENT_ID)
        .append_pair("redirect_uri", &callback::redirect_uri(CALLBACK_PORT))
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &attempt.challenge())
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &attempt.state)
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("originator", ORIGINATOR);
    inner.browser.open(&url)?;

    Ok(PendingSignIn {
        inner,
        listener,
        attempt,
    })
}

impl PendingSignIn {
    /// Waits for the callback, exchanges the code and saves the login.
    pub async fn finish(self) -> Result<Login, AuthError> {
        let Self {
            inner,
            listener,
            attempt,
        } = self;
        let callback = listener
            .wait()
            .await
            .map_err(|error| AuthError::Callback(error.to_string()))?;
        let param = |name: &str| callback.get(name).map(String::as_str);

        if param("state") != Some(attempt.state.as_str()) {
            return Err(AuthError::StateMismatch);
        }
        match param("error") {
            Some("access_denied") => return Err(AuthError::Declined),
            Some(error) => {
                return Err(AuthError::Authorization {
                    error: error.to_owned(),
                });
            }
            None => {}
        }
        let code = param("code")
            .ok_or_else(|| AuthError::Callback("the callback carries no code".to_owned()))?;

        let redirect_uri = callback::redirect_uri(CALLBACK_PORT);
        let tokens = oauth::token_request(
            &inner.http,
            &inner.endpoints.auth,
            &[
                ("grant_type", "authorization_code"),
                ("client_id", CODEX_CLIENT_ID),
                ("code", code),
                ("code_verifier", &attempt.verifier),
                ("redirect_uri", &redirect_uri),
            ],
        )
        .await
        .map_err(|failure| match failure {
            oauth::Failure::Network(message) => AuthError::Network(message),
            failure => AuthError::TokenEndpoint(failure.to_string()),
        })?;
        let id_token = tokens
            .id_token
            .ok_or_else(|| AuthError::IdTokenRejected("the exchange returned none".to_owned()))?;
        let identity = claims::identity(
            &id_token,
            &tokens.access_token,
            &oauth::issuer(&inner.endpoints.auth),
            CODEX_CLIENT_ID,
        )?;
        let refresh_token = tokens.refresh_token.ok_or_else(|| {
            AuthError::TokenEndpoint("the exchange returned no refresh token".to_owned())
        })?;

        let _serialised = inner.refresh_lock.lock().await;
        // A new sign-in replaces any saved login, a plan-sharing one included,
        // and clears a not-allowed report.
        let stored = StoredLogin::new(
            identity,
            tokens.access_token,
            refresh_token,
            inner.expires_at(tokens.expires_in),
        );
        stored.write(&*inner.secrets)?;
        Ok(stored.login())
    }
}
