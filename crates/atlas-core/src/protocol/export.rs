use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{GlossaryEntry, SectionKind, SectionVersion};

/// A course as it is printed: generated content only, nothing generated for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportDocument {
    pub course_id: Uuid,
    pub title: String,
    /// Every index entry in index order.
    pub contents: Vec<ContentsEntry>,
    /// Every section with a stored version, in index order.
    pub sections: Vec<ExportSection>,
    /// The appendix.
    pub glossary: Vec<GlossaryEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ContentsLevel {
    Chapter,
    Section,
    DeepDive,
}

/// A line of the table of contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContentsEntry {
    pub node_id: Uuid,
    pub level: ContentsLevel,
    /// 0 for a chapter, 1 for a broad-pass section, more for deep dives.
    pub depth: u32,
    pub title: String,
    /// False for an entry shown as its title only.
    pub included: bool,
}

/// A section in its current version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExportSection {
    pub node_id: Uuid,
    pub kind: SectionKind,
    pub depth: u32,
    pub title: String,
    pub summary: String,
    pub version: SectionVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ExportResult {
    Saved { path: String },
    Cancelled,
    Failed { reason: String },
}
