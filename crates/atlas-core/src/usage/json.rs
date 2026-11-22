//! Reading loosely typed answers the way Headroom reads them.

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;

/// `value` as non-empty text.
pub(super) fn text(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

/// `value` as a finite number: a number, numeric text or a boolean.
pub(super) fn number(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Number(number) => number.as_f64()?,
        Value::String(text) if text.is_empty() => return None,
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                0.0
            } else {
                text.parse().ok()?
            }
        }
        Value::Bool(flag) => f64::from(u8::from(*flag)),
        _ => return None,
    };
    number.is_finite().then_some(number)
}

/// `at` moved by `seconds`, if representable.
pub(super) fn after(at: DateTime<Utc>, seconds: f64) -> Option<DateTime<Utc>> {
    at.checked_add_signed(TimeDelta::try_milliseconds(milliseconds(seconds)?)?)
}

/// The moment `seconds` after the epoch, if representable.
pub(super) fn moment(seconds: f64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(milliseconds(seconds)?)
}

fn milliseconds(seconds: f64) -> Option<i64> {
    let milliseconds = (seconds * 1000.0).round();
    // `i64::MAX as f64` rounds up, so the bound is exclusive.
    (milliseconds.abs() < i64::MAX as f64).then_some(milliseconds as i64)
}

/// A plan as words: `self_serve_business_prolite` reads `Self serve business
/// prolite`.
pub(super) fn plan_words(plan: Option<&Value>) -> Option<String> {
    let words = text(plan)?.replace('_', " ");
    let mut chars = words.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}
