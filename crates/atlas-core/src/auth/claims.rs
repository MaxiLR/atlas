//! The claims of OpenAI's tokens. The tokens arrive straight from the token
//! endpoint over TLS, so their claims are read without checking a signature
//! (OpenID Connect Core 3.1.3.7); the ID token must still name the issuer and
//! the Codex client.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Map, Value};

use super::{AuthError, Identity};

/// The claim of OpenAI's profile, where an access token carries the email.
const PROFILE_CLAIM: &str = "https://api.openai.com/profile";

/// The payload of a JWT, or `None` when `token` is not one.
pub(crate) fn token_claims(token: &str) -> Option<Map<String, Value>> {
    let mut parts = token.split('.');
    let (Some(_), Some(payload), Some(_), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    match serde_json::from_slice(&bytes).ok()? {
        Value::Object(claims) => Some(claims),
        _ => None,
    }
}

fn rejected(reason: &str) -> AuthError {
    AuthError::IdTokenRejected(reason.to_owned())
}

fn text<'a>(claims: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    claims
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// The identity in `id_token`, issued by `issuer` for `client_id`; the email
/// may come from the access token's profile instead.
pub(super) fn identity(
    id_token: &str,
    access_token: &str,
    issuer: &str,
    client_id: &str,
) -> Result<Identity, AuthError> {
    let claims = token_claims(id_token).ok_or_else(|| rejected("it is not a JWT"))?;
    if text(&claims, "iss").map(|iss| iss.trim_end_matches('/')) != Some(issuer) {
        return Err(rejected("it names another issuer"));
    }
    let audience = match claims.get("aud") {
        Some(Value::String(audience)) => audience == client_id,
        Some(Value::Array(audiences)) => audiences
            .iter()
            .any(|audience| audience.as_str() == Some(client_id)),
        _ => false,
    };
    if !audience {
        return Err(rejected("it is meant for another client"));
    }
    let subject = text(&claims, "sub").ok_or_else(|| rejected("it names no subject"))?;
    let profile = token_claims(access_token);
    let email = text(&claims, "email")
        .or_else(|| {
            profile
                .as_ref()
                .and_then(|claims| claims.get(PROFILE_CLAIM))
                .and_then(Value::as_object)
                .and_then(|profile| text(profile, "email"))
        })
        .ok_or_else(|| rejected("it names no email"))?;
    Ok(Identity {
        subject: subject.to_owned(),
        email: email.to_owned(),
    })
}
