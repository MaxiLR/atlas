//! Checks and extends the course map ([`crate::domain::course_map`]) and
//! merges conventions so the first fixing holds.

use uuid::Uuid;

use crate::domain::conventions::Convention;
use crate::domain::course_map::{CourseMap, MapEntry};
use crate::domain::ladder::Ladder;
use crate::domain::record::StoredRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// The section has no map entry.
    MissingEntry,
    /// A concept relied on is introduced by no earlier section.
    NotIntroducedEarlier,
    /// A concept names a rung the ladder does not have.
    UnknownRung,
    EmptyName,
}

/// One fault of a map, in a form a repair request can quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFault {
    pub section: Uuid,
    pub concept: Option<String>,
    pub kind: FaultKind,
}

impl std::fmt::Display for MapFault {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

/// An incoming convention whose key is already fixed with another meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionConflict {
    pub fixed: Convention,
    pub incoming: Convention,
}

/// What [`merge_conventions`] makes of the incoming conventions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConventionMerge {
    pub accepted: Vec<Convention>,
    pub conflicts: Vec<ConventionConflict>,
}

pub fn validate(_map: &CourseMap, _index_order: &[Uuid], _ladder: &Ladder) -> Vec<MapFault> {
    todo!()
}

pub fn extend(
    _map: &CourseMap,
    _index_order: &[Uuid],
    _proposals: &CourseMap,
    _ladder: &Ladder,
) -> Result<CourseMap, Vec<MapFault>> {
    todo!()
}

pub fn entry(_map: &CourseMap, _section: Uuid) -> Option<&MapEntry> {
    todo!()
}

pub fn introducer(_map: &CourseMap, _index_order: &[Uuid], _concept: &str) -> Option<Uuid> {
    todo!()
}

pub fn prerequisite_sections(_map: &CourseMap, _index_order: &[Uuid], _section: Uuid) -> Vec<Uuid> {
    todo!()
}

pub fn merge_conventions(_existing: &[Convention], _incoming: &[Convention]) -> ConventionMerge {
    todo!()
}

pub fn predicted_record(_entry: &MapEntry) -> StoredRecord {
    todo!()
}
