//! One sign-in attempt: the authorisation page, the callback, the code
//! exchange and the ID token's validation.

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use url::Url;

use super::callback::CallbackListener;
use super::oauth::{self, AGENT_NAME, DYNAMIC_CLIENT, RESOURCE, SCOPES};
use super::stored::{StoredLogin, read_client_id, write_client_id};
use super::{AuthError, Inner, Login, SignInMode, id_token};

/// The per-attempt secrets: state, OIDC nonce and PKCE verifier.
struct Attempt {
    state: String,
    nonce: String,
    verifier: String,
}

impl Attempt {
    fn fresh() -> Self {
        let random = || URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
        Self {
            state: random(),
            nonce: random(),
            verifier: random(),
        }
    }

    /// The S256 challenge: the unpadded base64url SHA-256 of the verifier.
    fn challenge(&self) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(self.verifier.as_bytes()))
    }
}

/// A sign-in attempt waiting for its callback. Dropping it cancels it and
/// closes the listener.
pub struct PendingSignIn {
    inner: Arc<Inner>,
    listener: CallbackListener,
    attempt: Attempt,
    /// The saved client id this attempt reauthorises, or `None` for a new
    /// registration.
    client_id: Option<String>,
    mode: SignInMode,
}

pub(super) async fn start(inner: Arc<Inner>, mode: SignInMode) -> Result<PendingSignIn, AuthError> {
    let client_id = read_client_id(&*inner.secrets)?;
    if mode == SignInMode::EnablePlanUsage && client_id.is_none() {
        return Err(AuthError::NotRegistered);
    }
    let retained = StoredLogin::read(&*inner.secrets)?;
    let listener = CallbackListener::bind()
        .await
        .map_err(|error| AuthError::Callback(error.to_string()))?;
    let attempt = Attempt::fresh();

    let mut url = oauth::authorize_url(&inner.endpoints.auth);
    {
        let mut query = url.query_pairs_mut();
        match &client_id {
            None => {
                query
                    .append_pair("client_id", DYNAMIC_CLIENT)
                    .append_pair("agent_name_hint", AGENT_NAME);
            }
            Some(client_id) => {
                query.append_pair("client_id", client_id);
                if let Some(retained) = &retained {
                    query.append_pair("id_token_hint", &retained.id_token);
                }
            }
        }
        query
            .append_pair("ext_agent_host_id", &inner.host_id.0)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", listener.redirect_uri().as_str())
            .append_pair("scope", SCOPES)
            .append_pair("resource", RESOURCE)
            .append_pair("state", &attempt.state)
            .append_pair("nonce", &attempt.nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &attempt.challenge());
        if mode == SignInMode::EnablePlanUsage {
            query.append_pair("prompt", "consent");
        }
    }
    inner.browser.open(&url)?;

    Ok(PendingSignIn {
        inner,
        listener,
        attempt,
        client_id,
        mode,
    })
}

impl PendingSignIn {
    /// The exact callback URI this attempt sent.
    pub fn redirect_uri(&self) -> &Url {
        self.listener.redirect_uri()
    }

    /// Waits for the callback, exchanges the code and saves the login once its
    /// ID token is validated.
    pub async fn finish(self) -> Result<Login, AuthError> {
        let Self {
            inner,
            listener,
            attempt,
            client_id,
            mode,
        } = self;
        let redirect_uri = listener.redirect_uri().as_str().to_owned();
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
        let client_id = match client_id {
            None => {
                let issued = param("client_id")
                    .filter(|issued| !issued.is_empty() && *issued != DYNAMIC_CLIENT)
                    .ok_or(AuthError::RegistrationIncomplete)?;
                // Saved before the exchange, so a failed exchange keeps it.
                write_client_id(&*inner.secrets, issued)?;
                issued.to_owned()
            }
            Some(saved) => {
                if param("client_id").is_some_and(|named| named != saved) {
                    return Err(AuthError::ClientMismatch);
                }
                saved
            }
        };

        let discovery = oauth::discover(&inner.http, &inner.endpoints.auth)
            .await
            .map_err(|failure| AuthError::Discovery(failure.to_string()))?;
        let tokens = oauth::token_request(
            &inner.http,
            &inner.endpoints.auth,
            &[
                ("grant_type", "authorization_code"),
                ("client_id", &client_id),
                ("code", code),
                ("code_verifier", &attempt.verifier),
                ("redirect_uri", &redirect_uri),
                ("resource", RESOURCE),
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
        let identity = id_token::verify(
            &inner.http,
            &discovery,
            &id_token,
            &client_id,
            &attempt.nonce,
            inner.clock.now(),
        )
        .await?;
        let refresh_token = tokens.refresh_token.ok_or_else(|| {
            AuthError::TokenEndpoint("the exchange returned no refresh token".to_owned())
        })?;
        let scopes = tokens
            .scope
            .as_deref()
            .or(param("scope"))
            .map(oauth::scopes)
            .unwrap_or_default();

        let _serialised = inner.refresh_lock.lock().await;
        // Only a new sign-in clears a not-allowed report; enabling plan usage
        // for the same account keeps it.
        let plan_not_allowed = mode == SignInMode::EnablePlanUsage
            && StoredLogin::read(&*inner.secrets)?
                .is_some_and(|saved| saved.subject == identity.subject && saved.plan_not_allowed);
        let stored = StoredLogin {
            subject: identity.subject,
            email: identity.email,
            id_token,
            access_token: tokens.access_token,
            refresh_token,
            expires_at: inner.expires_at(tokens.expires_in),
            scopes,
            plan_not_allowed,
        };
        stored.write(&*inner.secrets)?;
        Ok(stored.login())
    }
}
