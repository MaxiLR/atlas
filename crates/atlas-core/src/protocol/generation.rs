use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{Anchor, Block, BlockKind, Citation, CoreError, Source, VersionStyle};

/// What a generation writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum GenerationTarget {
    /// A version of a section.
    Section {
        course_id: Uuid,
        section_id: Uuid,
        style: VersionStyle,
    },
    /// A definition of the anchored text.
    Define { course_id: Uuid, anchor: Anchor },
    /// A reply in a side chat.
    SideChat { course_id: Uuid, chat_id: Uuid },
}

/// What the agent is doing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum Activity {
    Writing,
    Searching,
    /// Starting over after a failed attempt; what arrived before is replaced.
    Retrying {
        attempt: u32,
    },
}

/// One step of a generation, in the order it happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum GenerationEvent {
    Started,
    Activity {
        activity: Activity,
    },
    /// A block began; until it completes it holds its place as a placeholder.
    BlockOpened {
        block: u32,
        kind: BlockKind,
    },
    /// Text appended to an open prose block.
    TextDelta {
        block: u32,
        text: String,
    },
    /// A block is final; it replaces what was shown for it.
    BlockCompleted {
        block: Block,
    },
    CitationAdded {
        citation: Citation,
        source: Source,
    },
    Finished {
        outcome: GenerationOutcome,
    },
}

/// How a generation ended. Anything but completed keeps what arrived, marked
/// incomplete, and stores nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum GenerationOutcome {
    Completed,
    Stopped,
    Failed {
        error: CoreError,
    },
    /// The provider ended the response early.
    Incomplete {
        reason: Option<String>,
    },
}
