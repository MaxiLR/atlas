//! The plan's usage a response of the Codex backend reports in its headers,
//! per window: the share used (0 to 100), the window's length in minutes and
//! its reset time (Unix seconds).

use chrono::DateTime;
use reqwest::header::HeaderMap;

use crate::provider::{UsageSnapshot, WindowUsage};

/// The headers of one window: share used, length, reset time.
type Names = [&'static str; 3];

const PRIMARY: Names = [
    "x-codex-primary-used-percent",
    "x-codex-primary-window-minutes",
    "x-codex-primary-reset-at",
];
const SECONDARY: Names = [
    "x-codex-secondary-used-percent",
    "x-codex-secondary-window-minutes",
    "x-codex-secondary-reset-at",
];

/// The usage `headers` report; none when no window's headers are all present
/// and readable.
pub(super) fn usage_snapshot(headers: &HeaderMap) -> Option<UsageSnapshot> {
    let snapshot = UsageSnapshot {
        primary: window(headers, PRIMARY),
        secondary: window(headers, SECONDARY),
    };
    (snapshot.primary.is_some() || snapshot.secondary.is_some()).then_some(snapshot)
}

/// The window these headers describe, when all three are present and
/// readable.
fn window(headers: &HeaderMap, [used, length, reset]: Names) -> Option<WindowUsage> {
    let value = |name| Some(headers.get(name)?.to_str().ok()?.trim());
    let percent = value(used)?
        .parse::<f64>()
        .ok()
        .filter(|percent| percent.is_finite())?;
    let window_minutes = value(length)?
        .parse::<u64>()
        .ok()
        .filter(|minutes| *minutes > 0)?;
    let resets_at = DateTime::from_timestamp(value(reset)?.parse().ok()?, 0)?;
    Some(WindowUsage {
        used: (percent / 100.0).clamp(0.0, 1.0),
        window_minutes,
        resets_at,
    })
}
