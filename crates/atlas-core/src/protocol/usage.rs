use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The usage meter, read with the plan login; a failed reading has no
/// consequence for anything else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum UsageState {
    /// There is no plan login to read with: the meter is empty.
    NoPlanLogin,
    /// Signed in, and no reading has arrived yet.
    AwaitingReading,
    /// The last reading, stale when a later one failed.
    Connected { reading: UsageReading },
    /// Signed in, but the first reading failed: there are no windows.
    FirstReadFailed { reason: UsageFailure },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageReading {
    /// The 5-hour and weekly windows, then any others the account reports.
    pub windows: Vec<UsageWindow>,
    pub taken_at: DateTime<Utc>,
    /// Why the latest reading failed, if it did.
    pub stale: Option<UsageFailure>,
}

/// One usage limit of the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// `5h`, `7d`, or the largest unit that divides the length.
    pub name: String,
    pub length_seconds: u32,
    /// The share used, from 0 to 1.
    pub used: f64,
    pub resets_at: Option<DateTime<Utc>>,
    pub level: UsageLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum UsageLevel {
    /// Below 75 %.
    Normal,
    /// From 75 %.
    High,
    /// From 90 %.
    Critical,
    /// At 100 %, or flagged by the provider.
    Limit,
}

/// Why a usage reading failed: a kind, never a response body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum UsageFailure {
    Network,
    Auth,
    Rejected,
    Timeout,
    Cancelled,
    Unexpected,
}
