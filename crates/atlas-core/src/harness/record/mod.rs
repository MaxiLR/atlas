//! Validation and repair of the section record a section closes with
//! ([`crate::domain::record`]), and the conventions it fixes.

use crate::domain::conventions::{Convention, ConventionOrigin};
use crate::domain::course_map::MapEntry;
use crate::domain::record::{SectionRecord, StoredRecord};
use crate::provider::ModelProvider;

pub const MAX_RECORD_CHARS: usize = 4_000;
pub const MAX_SUMMARY_CHARS: usize = 600;
pub const MAX_LINE_CHARS: usize = 300;
pub const MAX_ENTRIES: usize = 12;

/// Why a record was refused, quoted in the repair request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct RecordFault {
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedConventions {
    pub record: SectionRecord,
    pub fixed: Vec<Convention>,
}

pub fn validate(
    _raw: &str,
    _later_sections: &[impl AsRef<str>],
) -> Result<SectionRecord, RecordFault> {
    todo!("validate the section record")
}

pub async fn resolve(
    _raw: Option<&str>,
    _section_text: &str,
    _entry: &MapEntry,
    _later_sections: &[impl AsRef<str>],
    _provider: &dyn ModelProvider,
    _model: Option<&str>,
) -> StoredRecord {
    todo!("resolve the section record")
}

pub fn apply_conventions(
    _record: SectionRecord,
    _existing: &[Convention],
    _origin: ConventionOrigin,
) -> AppliedConventions {
    todo!("apply the record's conventions")
}
