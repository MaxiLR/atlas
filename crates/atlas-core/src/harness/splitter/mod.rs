//! Splits a section's stream into blocks as it arrives: Markdown prose, and a
//! reserved fenced block for everything that is not prose.

/// The reserved fence names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservedKind {
    /// `atlas-visual`: a json-render spec as JSON.
    Visual,
    /// `atlas-figure`: an SVG.
    Figure,
    /// `mermaid`.
    Mermaid,
    /// `atlas-check`: the check questions.
    Check,
    /// `atlas-record`: the closing section record, never shown.
    Record,
}

/// What the splitter reports, in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitEvent {
    /// Prose text for the shown block `block`.
    Prose { block: u32, text: String },
    /// A reserved fence opened. The record takes no shown index.
    Opened {
        block: Option<u32>,
        kind: ReservedKind,
    },
    /// A reserved fence closed with its payload and `alt:` description.
    Closed {
        block: Option<u32>,
        kind: ReservedKind,
        payload: String,
        description: Option<String>,
    },
    /// The input ended inside a reserved fence.
    Unclosed {
        block: Option<u32>,
        kind: ReservedKind,
        payload: String,
        description: Option<String>,
    },
    /// The input ended.
    Finished { record_seen: bool },
}

/// The incremental splitter.
#[derive(Debug, Default)]
pub struct Splitter {}

impl Splitter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, _chunk: &[u8]) -> Vec<SplitEvent> {
        Vec::new()
    }

    pub fn finish(self) -> Vec<SplitEvent> {
        Vec::new()
    }
}
