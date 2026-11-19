//! The context a side-chat reply is written from: the passage, its section's
//! full text, the index titles, the profile and the history, with older turns
//! condensed over budget. It never holds the course memory (ADR-0005,
//! ADR-0013): no map, no conventions and no records.

use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::harness::context::{Budget, render_index, render_profile};
use crate::index_tree::IndexTree;
use crate::protocol::ChatRole;

impl Budget {
    /// The context of a side-chat reply. It holds one section's full text, so
    /// it is sized like a section's.
    pub const SIDE_CHAT: Budget = Budget(48_000);
}

/// The longest opening a condensed turn keeps, in characters, when its text
/// has no sentence-ending mark sooner.
pub const OPENING_CHARS: usize = 280;

/// The marks that end a sentence when a space or the end of the text follows,
/// across scripts: Latin and general punctuation, CJK full-width forms,
/// Arabic, Urdu, Devanagari, Armenian, Ethiopic, Myanmar, Khmer, Mongolian,
/// Canadian syllabics and Lisu.
const SENTENCE_ENDS: &[char] = &[
    '.', '!', '?', '…', '‼', '‽', '⁇', '⁈', '⁉', '。', '．', '！', '？', '｡', '؟', '۔', '।', '॥',
    '։', '።', '፧', '။', '។', '៕', '᠃', '᠉', '᙮', '꓿',
];

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
    /// The full text of the passage's section version.
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
    /// How many of the oldest turns were condensed to fit the budget. A turn
    /// that is already its opening sentence is counted but left as it is.
    pub condensed: usize,
}

/// Between two parts of the input, and between two turns.
const SEPARATOR: &str = "\n\n";
const HISTORY_HEADING: &str = "# Conversation, oldest first";

impl ChatContext {
    /// The request's input: the parts in order.
    pub fn input(&self) -> String {
        self.parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(SEPARATOR)
    }

    pub fn part(&self, kind: ChatPartKind) -> Option<&str> {
        self.parts
            .iter()
            .find(|part| part.kind == kind)
            .map(|part| part.text.as_str())
    }
}

/// The context a side chat's next reply is written from: the quoted passage
/// and its section's full text when the chat is anchored, the index titles,
/// the profile and every turn in order, a stopped reply marked as stopped.
/// Over `budget`, the oldest turns are condensed to their opening sentence,
/// oldest first, stopping as soon as the input fits; the newest turn, the
/// passage and the profile are never shortened. No request is made.
pub fn side_chat_context(
    passage: Option<ChatPassage<'_>>,
    index: &IndexTree,
    ladder: &Ladder,
    profile: &CandidateProfile,
    history: &[ChatTurn],
    budget: Budget,
) -> ChatContext {
    let part = |kind, text| ChatPart { kind, text };
    let mut parts = Vec::new();
    if let Some(passage) = passage {
        parts.push(part(
            ChatPartKind::Passage,
            format!("# Passage the candidate selected\n{}", passage.quote),
        ));
        parts.push(part(
            ChatPartKind::Section,
            format!(
                "# The section it is from, in full\n{}",
                passage.section_text
            ),
        ));
    }
    parts.push(part(
        ChatPartKind::Index,
        format!("# Index\n{}", render_index(index)),
    ));
    parts.push(part(
        ChatPartKind::Profile,
        format!("# Candidate profile\n{}", render_profile(ladder, profile)),
    ));

    let mut turns: Vec<String> = history
        .iter()
        .map(|turn| render_turn(turn, &turn.text, false))
        .collect();
    let separator = SEPARATOR.chars().count();
    let mut size = parts
        .iter()
        .map(|part| part.text.chars().count() + separator)
        .sum::<usize>()
        + HISTORY_HEADING.chars().count()
        + turns
            .iter()
            .map(|turn| separator + turn.chars().count())
            .sum::<usize>();
    let mut condensed = 0;
    while size > budget.0 && condensed + 1 < history.len() {
        let turn = &history[condensed];
        let shorter = render_turn(turn, opening(&turn.text), true);
        let (before, after) = (turns[condensed].chars().count(), shorter.chars().count());
        if after < before {
            size = size - before + after;
            turns[condensed] = shorter;
        }
        condensed += 1;
    }

    if !turns.is_empty() {
        let mut text = HISTORY_HEADING.to_owned();
        for turn in turns {
            text.push_str(SEPARATOR);
            text.push_str(&turn);
        }
        parts.push(part(ChatPartKind::History, text));
    }
    ChatContext { parts, condensed }
}

/// A turn under its speaker, marked when stopped or condensed.
fn render_turn(turn: &ChatTurn, text: &str, condensed: bool) -> String {
    let speaker = match turn.role {
        ChatRole::Candidate => "Candidate",
        ChatRole::Agent => "Atlas",
    };
    let stopped = if turn.stopped {
        ", reply stopped before it finished"
    } else {
        ""
    };
    let condensed = if condensed { ", condensed" } else { "" };
    format!("## {speaker}{stopped}{condensed}\n{text}")
}

/// The text up to and including the first mark of [`SENTENCE_ENDS`] followed
/// by whitespace or the end, at most [`OPENING_CHARS`] characters.
fn opening(text: &str) -> &str {
    let mut chars = text.char_indices().peekable();
    let mut count = 0;
    while let Some((at, mark)) = chars.next() {
        if count == OPENING_CHARS {
            return &text[..at];
        }
        count += 1;
        let ends = chars.peek().is_none_or(|&(_, next)| next.is_whitespace());
        if SENTENCE_ENDS.contains(&mark) && ends {
            return &text[..at + mark.len_utf8()];
        }
    }
    text
}
