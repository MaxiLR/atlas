use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::CoreError;

/// A passage of one section version: a definition's term or a side chat's
/// quote. It shows only on that version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub version_id: Uuid,
    pub block: u32,
    pub quote: String,
}

/// A definition kept in the course glossary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryEntry {
    pub id: Uuid,
    pub course_id: Uuid,
    /// The section it was defined in.
    pub section_id: Uuid,
    pub section_title: String,
    pub term: String,
    pub definition: String,
    pub anchor: Anchor,
    /// The stored lookup, once one has been made.
    pub wikipedia: Option<WikipediaResult>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum WikipediaResult {
    Found {
        title: String,
        extract: String,
        url: String,
    },
    /// No article, or only a disambiguation page.
    NoArticle,
    Failed {
        error: CoreError,
    },
}
