use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A claim taken from a source: a character range of one prose block's
/// text, `start` and `end` counted in Unicode characters (scalar values),
/// not bytes or UTF-16 units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    pub block: u32,
    pub start: u32,
    pub end: u32,
    /// The number of its source.
    pub source: u32,
}

/// A web page cited, numbered in order of first citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub number: u32,
    pub url: String,
    pub title: String,
}
