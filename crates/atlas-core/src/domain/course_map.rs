//! The course map: what the agent plans with the index and does not show. For
//! every section, the concepts it introduces and the concepts it relies on.
//! Deep dives extend it.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ladder::RungId;

/// A concept of the course, named the same wherever it appears.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Concept {
    pub name: String,
    /// The ladder rung it belongs to, where it has one.
    pub rung: Option<RungId>,
}

/// One section's plan, by concept name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapEntry {
    /// The index node of the section.
    pub section: Uuid,
    pub introduces: Vec<String>,
    /// Each introduced by a section earlier in the index.
    pub relies_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseMap {
    pub concepts: Vec<Concept>,
    /// One per section, broad pass and deep dives.
    pub entries: Vec<MapEntry>,
}
