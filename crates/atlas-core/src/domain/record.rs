//! The section record: what a section leaves for later ones, written by the
//! agent in the section's closing block and never shown. One per version.

use serde::{Deserialize, Serialize};

/// The record as the model writes it. Its JSON is the payload of the closing
/// block; whether the core confirmed it is kept beside it, in
/// [`StoredRecord`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionRecord {
    /// Two to three sentences. Absent from a predicted record.
    pub summary: Option<String>,
    /// The concepts taught, each with one line as worded in the section.
    pub concepts: Vec<TaughtConcept>,
    /// Symbols fixed.
    pub notation: Vec<FixedEntry>,
    /// Terms fixed.
    pub terms: Vec<FixedEntry>,
    /// Examples and analogies used.
    pub examples: Vec<String>,
    /// What later sections were promised.
    pub promises: Vec<Promise>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaughtConcept {
    pub name: String,
    /// Absent from a predicted record.
    pub line: Option<String>,
}

/// A symbol or term and what it means in this course.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedEntry {
    pub key: String,
    pub meaning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promise {
    /// The later section's index key, such as `2.3`.
    pub section: String,
    pub what: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordStatus {
    /// Written by the model and validated.
    Confirmed,
    /// Predicted from the section's map entry after the model's record failed
    /// validation twice.
    Predicted,
}

/// A record as stored with its section version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredRecord {
    pub record: SectionRecord,
    pub status: RecordStatus,
}
