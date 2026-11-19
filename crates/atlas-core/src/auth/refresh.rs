//! Token refresh: one at a time, rotated tokens replacing the old ones
//! together, terminal errors ending the login.

use super::oauth::{self, Failure, RESOURCE, TERMINAL_REFRESH_CODES};
use super::stored::{StoredLogin, read_client_id};
use super::{AccessToken, Inner, RefreshError, Unreachable};

pub(super) async fn refresh(inner: &Inner) -> Result<AccessToken, RefreshError> {
    let seen = StoredLogin::read(&*inner.secrets)?.ok_or(RefreshError::SignedOut)?;
    let _serialised = inner.refresh_lock.lock().await;
    let mut login = StoredLogin::read(&*inner.secrets)?.ok_or(RefreshError::SignedOut)?;
    if login.refresh_token != seen.refresh_token {
        // Another caller refreshed while this one waited.
        return Ok(login.access_token());
    }
    let client_id = read_client_id(&*inner.secrets)?.ok_or(RefreshError::SignedOut)?;

    let answer = oauth::token_request(
        &inner.http,
        &inner.endpoints.auth,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("refresh_token", &login.refresh_token),
            ("resource", RESOURCE),
        ],
    )
    .await;
    let tokens = match answer {
        Ok(tokens) => tokens,
        Err(Failure::Network(message)) => {
            return Err(RefreshError::Unavailable(Unreachable::Network(message)));
        }
        Err(failure) if failure.is_transient() => {
            return Err(RefreshError::Unavailable(Unreachable::Answer(
                failure.to_string(),
            )));
        }
        Err(Failure::Status {
            code: Some(code), ..
        }) if TERMINAL_REFRESH_CODES.contains(&code.as_str()) => {
            StoredLogin::clear(&*inner.secrets)?;
            return Err(RefreshError::LoginEnded { code });
        }
        Err(Failure::Status { status, code }) => {
            return Err(RefreshError::Refused { status, code });
        }
        Err(failure) => {
            return Err(RefreshError::Unavailable(Unreachable::Answer(
                failure.to_string(),
            )));
        }
    };

    login.access_token = tokens.access_token;
    if let Some(refresh_token) = tokens.refresh_token {
        login.refresh_token = refresh_token;
    }
    login.expires_at = inner.expires_at(tokens.expires_in);
    if let Some(scope) = tokens.scope {
        login.scopes = oauth::scopes(&scope);
    }
    login.write(&*inner.secrets)?;
    Ok(login.access_token())
}
