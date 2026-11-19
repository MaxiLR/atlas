//! The context a section is written from, chosen by the core with no request
//! (ADR-0013), and its fixed shrink order over budget.

use std::collections::HashMap;

use uuid::Uuid;

use crate::domain::conventions::Convention;
use crate::domain::course_map::CourseMap;
use crate::domain::profile::CandidateProfile;
use crate::domain::record::StoredRecord;
use crate::index_tree::IndexTree;

/// What section context is built from. It holds no section's blocks, so
/// another section's full text cannot reach a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseMemory {
    pub profile: CandidateProfile,
    pub index: IndexTree,
    pub map: CourseMap,
    pub conventions: Vec<Convention>,
    /// The record of each section's current version, by index node.
    pub records: HashMap<Uuid, StoredRecord>,
}

/// A context budget in characters, an approximation of tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Budget(pub usize);
