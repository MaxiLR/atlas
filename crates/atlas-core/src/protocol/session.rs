use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::CoreError;

/// The plan login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Session {
    /// No plan login; `reason` says why when it ended or failed.
    SignedOut { reason: Option<SignedOutReason> },
    /// Waiting for the browser to come back.
    SigningIn,
    SignedIn {
        account: Account,
        plan_usage: PlanUsage,
    },
}

/// The ChatGPT account signed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub email: String,
}

/// Whether model calls may be paid with the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum PlanUsage {
    On,
    /// Signed in without the permission; it can be enabled.
    Off,
    /// The account or workspace may not use the plan; signing in again does
    /// not change it.
    NotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum SignedOutReason {
    /// The candidate signed out.
    SignedOut,
    /// The login was revoked or expired; the courses are kept.
    LoginEnded,
    /// Permission was declined in the browser.
    Declined,
    Failed {
        error: CoreError,
    },
}
