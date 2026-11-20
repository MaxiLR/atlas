//! The build-index task's output.

use serde::{Deserialize, Serialize};

/// The broad pass as the model returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDraft {
    pub chapters: Vec<ChapterDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterDraft {
    pub title: String,
    pub sections: Vec<SectionDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionDraft {
    pub title: String,
    pub summary: String,
}
