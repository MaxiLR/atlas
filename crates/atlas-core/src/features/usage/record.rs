//! The usage login as it is kept in the secret store: one JSON record of its
//! own, apart from the plan login.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::{AccessToken, RefreshToken};
use crate::usage::{UsageAccount, UsageLogin};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
    signed_in_at: DateTime<Utc>,
    account_id: String,
    email: Option<String>,
    plan: Option<String>,
}

pub(super) fn encode(login: &UsageLogin) -> String {
    let record = Record {
        access_token: login.access_token.secret().to_owned(),
        refresh_token: login.refresh_token.secret().to_owned(),
        expires_at: login.expires_at,
        signed_in_at: login.signed_in_at,
        account_id: login.account.account_id.clone(),
        email: login.account.email.clone(),
        plan: login.account.plan.clone(),
    };
    serde_json::to_string(&record).expect("a usage login serialises")
}

/// The login in `text`, or `None` when it is not a record this wrote.
pub(super) fn decode(text: &str) -> Option<UsageLogin> {
    let record: Record = serde_json::from_str(text).ok()?;
    Some(UsageLogin {
        access_token: AccessToken::new(record.access_token),
        refresh_token: RefreshToken::new(record.refresh_token),
        expires_at: record.expires_at,
        signed_in_at: record.signed_in_at,
        account: UsageAccount {
            account_id: record.account_id,
            email: record.email,
            plan: record.plan,
        },
    })
}
