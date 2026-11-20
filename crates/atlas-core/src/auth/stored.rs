//! The Codex login as the secret store keeps it. This is the only place
//! tokens are serialised.
//!
//! The record names the client that issued its tokens. A record without the
//! Codex client is the earlier plan-sharing login: its tokens belong to
//! another client, so it reads as no login and is never sent or refreshed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{AccessToken, CODEX_CLIENT_ID, Identity, Login};
use crate::ports::{SecretError, SecretRecord, SecretStore};

/// The login's record: the client, the identity, the tokens and whether the
/// provider reported the plan not allowed. No `Debug`: it holds tokens.
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredLogin {
    client_id: String,
    pub subject: String,
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default)]
    pub plan_not_allowed: bool,
}

impl StoredLogin {
    pub fn new(
        identity: Identity,
        access_token: String,
        refresh_token: String,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            client_id: CODEX_CLIENT_ID.to_owned(),
            subject: identity.subject,
            email: identity.email,
            access_token,
            refresh_token,
            expires_at,
            plan_not_allowed: false,
        }
    }

    /// The saved Codex login; `None` when there is none or the record was
    /// saved by plan sharing.
    pub fn read(secrets: &dyn SecretStore) -> Result<Option<Self>, SecretError> {
        let Some(value) = secrets.read(SecretRecord::PlanLogin)? else {
            return Ok(None);
        };
        let unreadable = || SecretError("the plan login record is unreadable".to_owned());
        let record: Value = serde_json::from_str(&value).map_err(|_| unreadable())?;
        if record.get("client_id").and_then(Value::as_str) != Some(CODEX_CLIENT_ID) {
            return Ok(None);
        }
        serde_json::from_value(record)
            .map(Some)
            .map_err(|_| unreadable())
    }

    pub fn write(&self, secrets: &dyn SecretStore) -> Result<(), SecretError> {
        let value = serde_json::to_string(self).expect("a login record serialises");
        secrets.write(SecretRecord::PlanLogin, &value)
    }

    /// Clears the saved login, whichever client saved it.
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
            expires_at: self.expires_at,
            plan_not_allowed: self.plan_not_allowed,
        }
    }
}
