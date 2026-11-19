//! Places the provider's citations, which index the whole output text, on the
//! prose blocks they fall in, and numbers the sources.

use crate::protocol::{Citation, Source};

/// A version's citations per block and its numbered sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacedCitations {
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
}
