//! The index as a tree the core reasons over: Next, index order, reading
//! states and counts (ADR-0004).

use uuid::Uuid;

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
