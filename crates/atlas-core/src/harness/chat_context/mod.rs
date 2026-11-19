//! The context a side-chat reply is written from: the passage, its section's
//! full text, the index titles, the profile and the history, with older turns
//! condensed over budget. It never holds the course memory.

use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::harness::context::Budget;
use crate::index_tree::IndexTree;
use crate::protocol::ChatRole;

impl Budget {
    /// The context of a side-chat reply.
    pub const SIDE_CHAT: Budget = Budget(48_000);
}

/// The longest opening a condensed turn keeps, in characters, when its text
/// has no sentence-ending mark sooner.
pub const OPENING_CHARS: usize = 280;

/// One turn of a side chat's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurn {
    pub role: ChatRole,
    pub text: String,
    /// A reply stopped before it finished.
    pub stopped: bool,
}

/// The passage an anchored chat was started from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatPassage<'a> {
    /// The quoted text.
    pub quote: &'a str,
    /// The full text of the section version the passage is in.
    pub section_text: &'a str,
}

/// The parts of a side chat's context, in the order a request carries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatPartKind {
    Passage,
    Section,
    Index,
    Profile,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPart {
    pub kind: ChatPartKind,
    pub text: String,
}

/// The input of a side-chat reply request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatContext {
    pub parts: Vec<ChatPart>,
    /// How many of the oldest turns were condensed to fit the budget.
    pub condensed: usize,
}

impl ChatContext {
    /// The request's input: the parts in order.
    pub fn input(&self) -> String {
        String::new()
    }

    pub fn part(&self, _kind: ChatPartKind) -> Option<&str> {
        None
    }
}

/// The context a side chat's next reply is written from.
pub fn side_chat_context(
    _passage: Option<ChatPassage<'_>>,
    _index: &IndexTree,
    _ladder: &Ladder,
    _profile: &CandidateProfile,
    _history: &[ChatTurn],
    _budget: Budget,
) -> ChatContext {
    ChatContext {
        parts: Vec::new(),
        condensed: 0,
    }
}
