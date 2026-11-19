//! Validation of the ID token against the authorisation server's published
//! keys: signature, issuer, audience, expiry and nonce.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use jsonwebtoken::jwk::{JwkSet, KeyAlgorithm};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use super::oauth::{self, Discovery};
use super::{AuthError, Identity};

/// Signature algorithms with a public key. HMAC is refused: its key would be
/// the published one.
const ALGORITHMS: &[Algorithm] = &[
    Algorithm::RS256,
    Algorithm::RS384,
    Algorithm::RS512,
    Algorithm::PS256,
    Algorithm::PS384,
    Algorithm::PS512,
    Algorithm::ES256,
    Algorithm::ES384,
    Algorithm::EdDSA,
];

/// Clock skew allowed on the expiry, in seconds.
const LEEWAY: i64 = 60;

#[derive(Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    nonce: Option<String>,
    exp: i64,
}

fn rejected(reason: impl ToString) -> AuthError {
    AuthError::IdTokenRejected(reason.to_string())
}

/// The identity in `token` once it is proven to come from the issuer, for
/// `client_id`, unexpired at `now` and bound to `nonce`.
pub(super) async fn verify(
    http: &reqwest::Client,
    discovery: &Discovery,
    token: &str,
    client_id: &str,
    nonce: &str,
    now: DateTime<Utc>,
) -> Result<Identity, AuthError> {
    let header = jsonwebtoken::decode_header(token).map_err(rejected)?;
    if !ALGORITHMS.contains(&header.alg) {
        return Err(rejected(format!(
            "the algorithm {:?} is refused",
            header.alg
        )));
    }
    let keys: JwkSet = oauth::get_json(http, discovery.jwks_uri.clone())
        .await
        .map_err(|failure| AuthError::Discovery(format!("the published keys: {failure}")))?;
    let jwk = match &header.kid {
        Some(kid) => keys.find(kid),
        None if keys.keys.len() == 1 => keys.keys.first(),
        None => None,
    }
    .ok_or_else(|| rejected("no published key matches"))?;
    if let Some(key_algorithm) = jwk.common.key_algorithm
        && key_algorithm != KeyAlgorithm::from(header.alg)
    {
        return Err(rejected("the key is published for another algorithm"));
    }
    let key = DecodingKey::from_jwk(jwk).map_err(rejected)?;

    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[discovery.issuer.as_str()]);
    validation.set_audience(&[client_id]);
    validation.required_spec_claims = ["exp", "iss", "aud", "sub"]
        .into_iter()
        .map(str::to_owned)
        .collect::<HashSet<_>>();
    // The expiry is checked against the core's clock below.
    validation.validate_exp = false;
    let claims = jsonwebtoken::decode::<Claims>(token, &key, &validation)
        .map_err(rejected)?
        .claims;

    if claims.exp + LEEWAY < now.timestamp() {
        return Err(rejected("the token has expired"));
    }
    if claims.nonce.as_deref() != Some(nonce) {
        return Err(rejected("the nonce does not match this attempt"));
    }
    let email = claims
        .email
        .filter(|email| !email.is_empty())
        .ok_or_else(|| rejected("the token names no email"))?;
    Ok(Identity {
        subject: claims.sub,
        email,
    })
}
