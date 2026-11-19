//! Validation of the blocks a section writes (catalog visual, figure, Mermaid,
//! check questions), one repair request, then the text fallback.

/// Why a block's payload was refused, quoted in the repair request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct BlockFault {
    pub reason: String,
}
