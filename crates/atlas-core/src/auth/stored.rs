//! The plan login and the issued client id as the secret store keeps them.
//! This is the only place tokens are serialised.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::oauth::PLAN_SCOPE;
use super::{AccessToken, Identity, Login};
use crate::ports::{SecretError, SecretRecord, SecretStore};

/// The plan login's record: the validated identity, the tokens, the granted
/// scopes and the ID token kept for `id_token_hint`. No `Debug`: it holds
/// tokens.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredLogin {
    pub subject: String,
    pub email: String,
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub scopes: Vec<String>,
}

impl StoredLogin {
    pub fn read(secrets: &dyn SecretStore) -> Result<Option<Self>, SecretError> {
        secrets
            .read(SecretRecord::PlanLogin)?
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|_| SecretError("the plan login record is unreadable".to_owned()))
            })
            .transpose()
    }

    pub fn write(&self, secrets: &dyn SecretStore) -> Result<(), SecretError> {
        let value = serde_json::to_string(self).expect("a login record serialises");
        secrets.write(SecretRecord::PlanLogin, &value)
    }

    /// Clears the tokens; the issued client id and the host id stay.
    pub fn clear(secrets: &dyn SecretStore) -> Result<(), SecretError> {
        secrets.delete(SecretRecord::PlanLogin)
    }

    pub fn access_token(&self) -> AccessToken {
        AccessToken::new(self.access_token.clone())
    }

    pub fn login(&self) -> Login {
        Login {
            identity: Identity {
                subject: self.subject.clone(),
                email: self.email.clone(),
            },
            scopes: self.scopes.clone(),
            plan_usage: self.scopes.iter().any(|scope| scope == PLAN_SCOPE),
            expires_at: self.expires_at,
        }
    }
}

pub(super) fn read_client_id(secrets: &dyn SecretStore) -> Result<Option<String>, SecretError> {
    Ok(secrets
        .read(SecretRecord::IssuedClientId)?
        .filter(|client_id| !client_id.is_empty()))
}

pub(super) fn write_client_id(
    secrets: &dyn SecretStore,
    client_id: &str,
) -> Result<(), SecretError> {
    secrets.write(SecretRecord::IssuedClientId, client_id)
}
