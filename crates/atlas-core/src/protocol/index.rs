use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// A chapter of the broad pass: a station on the index line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChapterNode {
    pub id: Uuid,
    pub title: String,
    pub sections: Vec<SectionNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum SectionKind {
    /// A section of the broad pass, under a chapter.
    BroadPass,
    /// A child section made by Go deeper, at any depth.
    DeepDive,
}

/// How a section's node is drawn: a mark, not a colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum NodeMark {
    Unread,
    Read,
    /// The section has deep dives.
    Branch,
}

/// A section of the index with its deep dives, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SectionNode {
    pub id: Uuid,
    pub kind: SectionKind,
    pub title: String,
    /// One line on what the section covers.
    pub summary: String,
    /// A complete version has been opened or finished generating in front of
    /// the candidate.
    pub read: bool,
    pub mark: NodeMark,
    pub deep_dives: Vec<SectionNode>,
}
