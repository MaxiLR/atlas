//! The conventions a course has fixed. The first fixing holds for the whole
//! course.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConventionKind {
    Notation,
    Terminology,
    RunningExample,
}

/// Where a convention was first fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConventionOrigin {
    /// The plan made with the index.
    Plan,
    /// The record of a section version.
    SectionVersion { version: Uuid },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Convention {
    pub kind: ConventionKind,
    /// The symbol, term or example's name; unique per course and kind.
    pub key: String,
    pub meaning: String,
    pub origin: ConventionOrigin,
}
