//! A usage reading: `GET {chatgpt}/backend-api/wham/usage`, its windows named
//! by their length, and how full each one is.

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use super::json::{after, moment, number, plan_words, text};
use super::{Usage, UsageLogin, UsageReader, Window, object_of, send, under};
use crate::protocol::{UsageFailure, UsageLevel};

const USAGE: &[&str] = &["backend-api", "wham", "usage"];

/// Window lengths with a name of their own: seconds, id, label.
const NAMED_LENGTHS: &[(u64, &str, &str)] = &[(18_000, "5h", "5-hour"), (604_800, "7d", "Weekly")];
/// Other lengths are named in the largest of these units that divides them.
const UNITS: &[(u64, &str, &str)] = &[
    (86_400, "d", "day"),
    (3_600, "h", "hour"),
    (60, "m", "minute"),
    (1, "s", "second"),
];

/// The id and label of a window.
struct Name {
    id: String,
    label: String,
}

impl UsageReader {
    /// Reads the account's usage windows. A 401 ends the login (`Auth`).
    pub async fn read(&self, login: &UsageLogin) -> Result<Usage, UsageFailure> {
        let response = send(
            self.http
                .get(under(&self.endpoints.chatgpt, USAGE))
                .bearer_auth(login.access_token.secret())
                .header("ChatGPT-Account-Id", &login.account.account_id),
        )
        .await?;
        if response.status().as_u16() == 401 {
            return Err(UsageFailure::Auth);
        }
        let usage = object_of(response).await?;
        let now = self.clock.now();
        let mut windows = windows_of(usage.get("rate_limit"), now, |name| name);
        if let Some(additional) = usage
            .get("additional_rate_limits")
            .and_then(Value::as_array)
        {
            for entry in additional {
                windows.extend(additional_windows(entry, now));
            }
        }
        Ok(Usage {
            windows,
            plan: plan_words(usage.get("plan_type")),
        })
    }
}

/// How full `window` is.
pub fn level_of(window: &Window) -> UsageLevel {
    if window.at_limit || window.used >= 1.0 {
        UsageLevel::Limit
    } else if window.used >= 0.9 {
        UsageLevel::Critical
    } else if window.used >= 0.75 {
        UsageLevel::High
    } else {
        UsageLevel::Normal
    }
}

/// The name of a window `seconds` long.
fn length_name(seconds: u64) -> Name {
    if let Some((_, id, label)) = NAMED_LENGTHS.iter().find(|(length, ..)| *length == seconds) {
        return Name {
            id: (*id).to_owned(),
            label: (*label).to_owned(),
        };
    }
    let (unit, id, label) = UNITS
        .iter()
        .find(|(unit, ..)| seconds.is_multiple_of(*unit))
        .expect("every length is a whole number of seconds");
    let count = seconds / unit;
    Name {
        id: format!("{count}{id}"),
        label: format!("{count}-{label}"),
    }
}

/// The windows of one rate limit, longest first, named by `naming`; a
/// reached limit marks the fullest window and any at 100 %.
fn windows_of(
    limit: Option<&Value>,
    now: DateTime<Utc>,
    naming: impl Fn(Name) -> Name,
) -> Vec<Window> {
    let Some(limit) = limit.and_then(Value::as_object) else {
        return Vec::new();
    };
    let slots: Vec<(&Map<String, Value>, u64, f64)> = ["primary_window", "secondary_window"]
        .into_iter()
        .filter_map(|key| {
            let slot = limit.get(key)?.as_object()?;
            let seconds = number(slot.get("limit_window_seconds"))
                .filter(|seconds| seconds.fract() == 0.0 && *seconds > 0.0)?;
            let percent = number(slot.get("used_percent")).unwrap_or(0.0);
            // Saturates at `u64::MAX`, beyond any real window.
            Some((slot, seconds as u64, percent))
        })
        .collect();
    let fullest = slots
        .iter()
        .map(|(_, _, percent)| *percent)
        .fold(f64::NEG_INFINITY, f64::max);
    let reached = limit.get("limit_reached") == Some(&Value::Bool(true));
    let mut windows: Vec<Window> = slots
        .into_iter()
        .map(|(slot, seconds, percent)| {
            let Name { id, label } = naming(length_name(seconds));
            let resets_at = match number(slot.get("reset_at")) {
                Some(at) => moment(at),
                None => {
                    number(slot.get("reset_after_seconds")).and_then(|seconds| after(now, seconds))
                }
            };
            Window {
                id,
                label,
                length_seconds: seconds,
                used: (percent / 100.0).clamp(0.0, 1.0),
                resets_at,
                at_limit: reached && (percent == fullest || percent >= 100.0),
            }
        })
        .collect();
    // Stable, as Headroom's: equal lengths keep the slots' order.
    windows.sort_by_key(|window| std::cmp::Reverse(window.length_seconds));
    windows
}

/// The windows of a named additional rate limit.
fn additional_windows(entry: &Value, now: DateTime<Utc>) -> Vec<Window> {
    let Some(name) = text(entry.get("limit_name")) else {
        return Vec::new();
    };
    let slug = slug(name);
    windows_of(entry.get("rate_limit"), now, |length| Name {
        id: format!("extra:{slug}:{}", length.id),
        label: format!("{name} · {}", length.label),
    })
}

/// `name` in lower case with each run of other characters than `a-z` and
/// `0-9` as one hyphen: `GPT-5.3-Codex-Spark` reads `gpt-5-3-codex-spark`.
fn slug(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    let mut in_run = false;
    for char in name.to_lowercase().chars() {
        if char.is_ascii_lowercase() || char.is_ascii_digit() {
            slug.push(char);
            in_run = false;
        } else if !in_run {
            slug.push('-');
            in_run = true;
        }
    }
    slug
}
