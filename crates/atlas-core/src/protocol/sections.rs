use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{Citation, Source};
use crate::sanitise::SanitisedSvg;

/// How a version was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VersionStyle {
    /// The first version of the section.
    Original,
    Simpler,
    MoreTechnical,
    MoreExamples,
}

/// The ways "Explain differently" can rewrite a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ExplainStyle {
    Simpler,
    MoreTechnical,
    MoreExamples,
}

/// A section with its current version and the list of its versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    /// One line on what the section covers.
    pub summary: String,
    pub version: SectionVersion,
    /// Every version, oldest first.
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VersionSummary {
    pub id: Uuid,
    pub style: VersionStyle,
    pub created_at: DateTime<Utc>,
}

/// One stored, immutable version of a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SectionVersion {
    pub id: Uuid,
    pub style: VersionStyle,
    pub blocks: Vec<Block>,
    pub citations: Vec<Citation>,
    /// Numbered in order of first citation.
    pub sources: Vec<Source>,
    /// The account's policy refused web search, so none was used.
    pub written_without_search: bool,
    /// The model that wrote it.
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One unit of a section's content. `index` is stable within its version, so
/// anchors and citations can name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub index: u32,
    pub content: BlockContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum BlockContent {
    /// Markdown, rendered without raw HTML.
    Prose { text: String },
    /// A json-render spec composed from the catalog, as JSON.
    Visual {
        #[ts(type = "unknown")]
        spec: serde_json::Value,
        description: Option<String>,
    },
    /// A static SVG that passed the core's sanitiser.
    Figure {
        svg: SanitisedSvg,
        description: Option<String>,
    },
    Mermaid {
        source: String,
        description: Option<String>,
    },
    /// One or two check questions.
    Check { questions: Vec<CheckQuestion> },
    /// A visual that failed validation twice, shown as its description.
    Fallback { description: String },
}

/// The kind of a block opened while a section streams, before it completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum BlockKind {
    Prose,
    Visual,
    Figure,
    Mermaid,
    Check,
}

/// A check question as shown. Which option is right is told only once it has
/// been answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CheckQuestion {
    pub index: u32,
    pub stem: String,
    pub options: Vec<String>,
    pub answer: Option<CheckResult>,
}

/// The outcome of answering a check question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    /// The option chosen, counted from 0.
    pub choice: u32,
    pub correct: bool,
    /// The right option, counted from 0.
    pub key: u32,
    pub explanation: String,
}
