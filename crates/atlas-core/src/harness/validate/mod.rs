//! Validation of the blocks a section writes (catalog visual, figure, Mermaid,
//! check questions), one repair request, then the text fallback.

use std::sync::atomic::AtomicBool;

use serde_json::Value;

use crate::domain::ladder::Ladder;
use crate::domain::questions::KeyedCheckQuestion;
use crate::harness::splitter::ReservedKind;
use crate::protocol::BlockContent;
use crate::provider::ModelProvider;
use crate::sanitise::SanitisedSvg;

/// The largest Mermaid source accepted, in bytes.
pub const MAX_MERMAID_BYTES: usize = 16 * 1024;

/// What a fallback shows when its block came with no description.
pub const NEUTRAL_DESCRIPTION: &str = "This part of the section could not be shown.";

/// Why a block's payload was refused, quoted in the repair request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct BlockFault {
    pub reason: String,
}

/// The section record was handed to this module: a programming error, since
/// `harness::record` validates, repairs and replaces the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the section record is not a shown block: harness::record resolves it")]
pub struct RecordBlock;

/// A shown block as the splitter reported it: its kind, its raw payload, its
/// `alt:` description, and whether its fence closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBlock {
    kind: ReservedKind,
    payload: String,
    description: Option<String>,
    closed: bool,
}

impl RawBlock {
    /// A block whose fence closed.
    pub fn closed(
        kind: ReservedKind,
        payload: impl Into<String>,
        description: Option<String>,
    ) -> Result<Self, RecordBlock> {
        let _ = (kind, payload.into(), description);
        todo!()
    }

    /// A block still open when the stream ended.
    pub fn unclosed(
        kind: ReservedKind,
        payload: impl Into<String>,
        description: Option<String>,
    ) -> Result<Self, RecordBlock> {
        let _ = (kind, payload.into(), description);
        todo!()
    }
}

/// A block that passed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidBlock {
    /// A json-render spec that passed the catalog.
    Visual {
        spec: Value,
    },
    Figure {
        svg: SanitisedSvg,
    },
    Mermaid {
        source: String,
    },
    /// One or two check questions, with their keys.
    Check {
        questions: Vec<KeyedCheckQuestion>,
    },
}

/// What a block resolves to: valid, possibly after its repair, or the text
/// fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Block {
        block: ValidBlock,
        description: Option<String>,
    },
    Fallback {
        description: String,
    },
}

impl Resolved {
    /// The block as the interface shows it; check questions without their
    /// keys.
    pub fn content(&self) -> BlockContent {
        todo!()
    }
}

/// Checks a block's payload.
pub fn validate(raw: &RawBlock, ladder: &Ladder) -> Result<ValidBlock, BlockFault> {
    let _ = (raw, ladder);
    todo!()
}

/// Resolves a block: valid, it is the block; invalid, one repair request.
pub async fn resolve(
    raw: &RawBlock,
    ladder: &Ladder,
    provider: &dyn ModelProvider,
    model: Option<&str>,
    stopped: &AtomicBool,
) -> Resolved {
    let _ = (raw, ladder, provider, model, stopped);
    todo!()
}
