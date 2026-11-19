//! The context a section is written from, chosen by the core with no request
//! (ADR-0013), and its fixed shrink order over budget.

use std::collections::HashMap;

use uuid::Uuid;

use crate::domain::conventions::Convention;
use crate::domain::course_map::CourseMap;
use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::domain::record::StoredRecord;
use crate::index_tree::IndexTree;

/// What section context is built from. It holds no section's blocks, so
/// another section's full text cannot reach a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseMemory {
    pub ladder: Ladder,
    pub profile: CandidateProfile,
    /// The index with each section's reading state.
    pub index: IndexTree,
    pub map: CourseMap,
    pub conventions: Vec<Convention>,
    /// The record of each section's current version, by index node.
    pub records: HashMap<Uuid, StoredRecord>,
}

/// A context budget in characters, an approximation of tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Budget(pub usize);

impl Budget {
    /// The context of a section request.
    pub const SECTION: Budget = Budget(48_000);
}

/// The size limits a confirmed section record is validated against, in
/// characters. The section budget is sized from them.
pub const RECORD_CHARS: usize = 4_000;
/// The record's summary.
pub const RECORD_SUMMARY_CHARS: usize = 600;
/// One concept line, meaning, example or promise.
pub const RECORD_LINE_CHARS: usize = 300;
/// The entries of each of the record's lists.
pub const RECORD_ENTRIES: usize = 12;

/// The candidate profile as a request carries it.
pub fn render_profile(_ladder: &Ladder, _profile: &CandidateProfile) -> String {
    String::new()
}

/// The short stable key of each index entry: `2` for a chapter, `2.3` for a
/// section and `2.3.1` for a deep dive under it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexKeys {
    by_node: HashMap<Uuid, String>,
    by_key: HashMap<String, Uuid>,
}

impl IndexKeys {
    pub fn new(_tree: &IndexTree) -> Self {
        Self::default()
    }

    pub fn key(&self, node: Uuid) -> Option<&str> {
        self.by_node.get(&node).map(String::as_str)
    }

    pub fn node(&self, key: &str) -> Option<Uuid> {
        self.by_key.get(key).copied()
    }
}

/// The index titles with their keys and their place in the tree.
pub fn render_index(_tree: &IndexTree) -> String {
    String::new()
}

/// The parts of a section's context, in the order a request carries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    Profile,
    Index,
    Conventions,
    /// The section's own map entry.
    Entry,
    /// The concepts relied on and the records of the sections introducing
    /// them.
    Prerequisites,
    /// The record of the section before it in index order.
    Previous,
    /// What earlier sections promised this one would cover.
    Promises,
    /// A deep dive's parent's record.
    Parent,
    /// Every other section read, under its chapter.
    OtherRead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPart {
    pub kind: PartKind,
    pub text: String,
}

/// A shrinking step, in the order they are applied over budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shrink {
    /// The other sections read become their chapters' titles only.
    ChapterTitles,
    /// The previous section's record becomes its summary.
    PreviousSummary,
    /// Each prerequisite record becomes the lines of the concepts relied on.
    PrerequisiteLines,
}

/// The input of a section request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionContext {
    pub parts: Vec<ContextPart>,
    /// The steps applied to fit the budget, in order.
    pub shrunk: Vec<Shrink>,
}

impl SectionContext {
    /// The request's input: the parts in order.
    pub fn input(&self) -> String {
        String::new()
    }

    pub fn part(&self, kind: PartKind) -> Option<&str> {
        self.parts
            .iter()
            .find(|part| part.kind == kind)
            .map(|part| part.text.as_str())
    }
}

/// The context `node` is written from. None when `node` is not a section of
/// the index.
pub fn section_context(
    _memory: &CourseMemory,
    _node: Uuid,
    _budget: Budget,
) -> Option<SectionContext> {
    None
}
