//! The index as a tree the core reasons over: Next, index order, reading
//! states and counts (ADR-0004).

use thiserror::Error;
use uuid::Uuid;

use crate::protocol::{NodeMark, Progress, SectionKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexTree {
    pub chapters: Vec<TreeChapter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeChapter {
    pub id: Uuid,
    pub title: String,
    pub sections: Vec<TreeSection>,
}

/// A section of the broad pass or a deep dive, with its children in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSection {
    pub id: Uuid,
    pub title: String,
    /// One line on what it covers.
    pub line: String,
    pub read: bool,
    pub children: Vec<TreeSection>,
}

/// A section as the index shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexEntry<'a> {
    pub chapter: Uuid,
    pub kind: SectionKind,
    /// 0 for the broad pass, 1 for a deep dive under it, and so on.
    pub depth: u32,
    pub section: &'a TreeSection,
}

/// How a section's node is drawn, and whether it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadingState {
    pub mark: NodeMark,
    pub read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TreeError {
    #[error("no section {0} in the index")]
    UnknownSection(Uuid),
    #[error("a deep dive has three to six sections, not {0}")]
    ChildCount(usize),
    #[error("{0} is already in the index")]
    DuplicateId(Uuid),
}

pub fn next(_tree: &IndexTree, _node: Uuid) -> Option<Uuid> {
    None
}

pub fn in_index_order(_tree: &IndexTree) -> Vec<IndexEntry<'_>> {
    Vec::new()
}

pub fn state(_tree: &IndexTree, _section: Uuid) -> Option<ReadingState> {
    None
}

pub fn mark_read(_tree: &mut IndexTree, _section: Uuid) -> bool {
    false
}

pub fn progress(_tree: &IndexTree) -> Progress {
    Progress { read: 0, total: 0 }
}

pub fn furthest_read(_tree: &IndexTree) -> Option<Uuid> {
    None
}

pub fn first_section(_tree: &IndexTree) -> Option<Uuid> {
    None
}

pub fn first_section_of(_tree: &IndexTree, _chapter: Uuid) -> Option<Uuid> {
    None
}

pub fn insert_children(
    _tree: &mut IndexTree,
    _parent: Uuid,
    _children: Vec<TreeSection>,
) -> Result<(), TreeError> {
    Ok(())
}
