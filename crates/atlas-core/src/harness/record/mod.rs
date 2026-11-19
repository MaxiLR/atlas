//! Validation and repair of the section record a section closes with
//! ([`crate::domain::record`]), and the conventions it fixes.

/// Why a record was refused, quoted in the repair request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct RecordFault {
    pub reason: String,
}
