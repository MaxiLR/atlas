//! Sign-out: revoke the refresh token at the discovery document's revocation
//! endpoint, then clear the tokens, keeping the client id and the host id.

use std::time::Duration;

use super::oauth::{self, Failure};
use super::stored::{StoredLogin, read_client_id};
use super::{AuthError, Inner, Revocation, SignedOut};

/// The pauses before each retry of a revocation that failed transiently.
const BACKOFF: &[Duration] = &[Duration::from_secs(1), Duration::from_secs(2)];

pub(super) async fn sign_out(inner: &Inner) -> Result<SignedOut, AuthError> {
    let _serialised = inner.refresh_lock.lock().await;
    let Some(login) = StoredLogin::read(&*inner.secrets)? else {
        return Ok(SignedOut {
            revocation: Revocation::NothingToRevoke,
        });
    };
    let revocation = match read_client_id(&*inner.secrets)? {
        Some(client_id) => revoke(inner, &login.refresh_token, &client_id).await,
        None => Revocation::Unconfirmed {
            reason: "there is no issued client id to revoke with".to_owned(),
        },
    };
    StoredLogin::clear(&*inner.secrets)?;
    Ok(SignedOut { revocation })
}

async fn revoke(inner: &Inner, refresh_token: &str, client_id: &str) -> Revocation {
    let mut pauses = BACKOFF.iter();
    loop {
        let failure = match attempt(inner, refresh_token, client_id).await {
            Ok(()) => return Revocation::Confirmed,
            Err(failure) => failure,
        };
        match pauses.next() {
            Some(pause) if failure.is_transient() => inner.clock.sleep(*pause).await,
            _ => {
                return Revocation::Unconfirmed {
                    reason: failure.to_string(),
                };
            }
        }
    }
}

async fn attempt(inner: &Inner, refresh_token: &str, client_id: &str) -> Result<(), Failure> {
    let discovery = oauth::discover(&inner.http, &inner.endpoints.auth).await?;
    let endpoint = discovery.revocation_endpoint.ok_or_else(|| {
        Failure::Body("the discovery document names no revocation endpoint".to_owned())
    })?;
    oauth::post_form(
        &inner.http,
        endpoint,
        &[
            ("token", refresh_token),
            ("token_type_hint", "refresh_token"),
            ("client_id", client_id),
        ],
    )
    .await?;
    Ok(())
}
