//! Checks and extends the course map ([`crate::domain::course_map`]) and
//! merges conventions so the first fixing holds.

use uuid::Uuid;

use crate::domain::conventions::Convention;

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

/// An incoming convention whose key is already fixed with another meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionConflict {
    pub fixed: Convention,
    pub incoming: Convention,
}
