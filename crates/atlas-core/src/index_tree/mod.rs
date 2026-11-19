//! The index as a tree the core reasons over: Next, index order, reading
//! states and counts (ADR-0004).

use std::collections::HashSet;

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

/// A deep dive has three to six sections.
pub const DEEP_DIVE_SIZE: std::ops::RangeInclusive<usize> = 3..=6;

impl TreeSection {
    /// How the section's node is drawn: a branch once it has deep dives,
    /// otherwise read or unread.
    pub fn mark(&self) -> NodeMark {
        if !self.children.is_empty() {
            NodeMark::Branch
        } else if self.read {
            NodeMark::Read
        } else {
            NodeMark::Unread
        }
    }
}

/// The section Next opens from `node`, never a chapter. From a broad-pass
/// section, the following one, crossing chapters. From a deep dive, its next
/// sibling; after the last sibling, at any depth, the broad-pass section
/// that follows the one the branch hangs from. From a chapter, its first
/// section. None after the last section of the course, or for an id not in
/// the index.
pub fn next(tree: &IndexTree, node: Uuid) -> Option<Uuid> {
    if let Some(chapter) = tree.chapters.iter().position(|chapter| chapter.id == node) {
        return broad_pass_from(tree, chapter, 0)
            .next()
            .map(|section| section.id);
    }
    let mut path = Vec::new();
    let chapter = tree
        .chapters
        .iter()
        .position(|chapter| path_to(&chapter.sections, node, &mut path))?;
    let (&at, ancestors) = path.split_last()?;
    if let Some((&root, between)) = ancestors.split_first() {
        let parent = between
            .iter()
            .fold(&tree.chapters[chapter].sections[root], |section, &child| {
                &section.children[child]
            });
        if let Some(sibling) = parent.children.get(at + 1) {
            return Some(sibling.id);
        }
    }
    broad_pass_from(tree, chapter, path[0] + 1)
        .next()
        .map(|section| section.id)
}

/// Every section in the order the index shows it: each broad-pass section
/// followed by its deep dives, depth first.
pub fn in_index_order(tree: &IndexTree) -> Vec<IndexEntry<'_>> {
    fn walk<'a>(
        chapter: Uuid,
        depth: u32,
        sections: &'a [TreeSection],
        out: &mut Vec<IndexEntry<'a>>,
    ) {
        for section in sections {
            out.push(IndexEntry {
                chapter,
                kind: if depth == 0 {
                    SectionKind::BroadPass
                } else {
                    SectionKind::DeepDive
                },
                depth,
                section,
            });
            walk(chapter, depth + 1, &section.children, out);
        }
    }
    let mut entries = Vec::new();
    for chapter in &tree.chapters {
        walk(chapter.id, 0, &chapter.sections, &mut entries);
    }
    entries
}

/// The reading state of a section; None for a chapter or an unknown id.
pub fn state(tree: &IndexTree, section: Uuid) -> Option<ReadingState> {
    find(tree, section).map(|section| ReadingState {
        mark: section.mark(),
        read: section.read,
    })
}

/// Marks a section read. False when no section has that id.
pub fn mark_read(tree: &mut IndexTree, section: Uuid) -> bool {
    find_mut(tree, section)
        .map(|section| section.read = true)
        .is_some()
}

/// Sections read out of the sections present, broad pass and deep dives.
pub fn progress(tree: &IndexTree) -> Progress {
    in_index_order(tree)
        .iter()
        .fold(Progress { read: 0, total: 0 }, |progress, entry| Progress {
            read: progress.read + u32::from(entry.section.read),
            total: progress.total + 1,
        })
}

/// The last broad-pass section read: the line is bright up to it.
pub fn furthest_read(tree: &IndexTree) -> Option<Uuid> {
    broad_pass_from(tree, 0, 0)
        .filter(|section| section.read)
        .last()
        .map(|section| section.id)
}

/// The section a course starts at.
pub fn first_section(tree: &IndexTree) -> Option<Uuid> {
    broad_pass_from(tree, 0, 0).next().map(|section| section.id)
}

/// The section a chapter opens at; None for anything but a chapter with
/// sections.
pub fn first_section_of(tree: &IndexTree, chapter: Uuid) -> Option<Uuid> {
    tree.chapters
        .iter()
        .find(|candidate| candidate.id == chapter)?
        .sections
        .first()
        .map(|section| section.id)
}

/// Adds a deep dive of three to six sections under `parent`, after any
/// children it already has, in the order given. On a refusal the tree is
/// unchanged.
pub fn insert_children(
    tree: &mut IndexTree,
    parent: Uuid,
    children: Vec<TreeSection>,
) -> Result<(), TreeError> {
    if find(tree, parent).is_none() {
        return Err(TreeError::UnknownSection(parent));
    }
    if !DEEP_DIVE_SIZE.contains(&children.len()) {
        return Err(TreeError::ChildCount(children.len()));
    }
    fn first_taken(sections: &[TreeSection], ids: &mut HashSet<Uuid>) -> Option<Uuid> {
        sections.iter().find_map(|section| {
            if ids.insert(section.id) {
                first_taken(&section.children, ids)
            } else {
                Some(section.id)
            }
        })
    }
    let mut ids: HashSet<Uuid> = tree.chapters.iter().map(|chapter| chapter.id).collect();
    ids.extend(in_index_order(tree).iter().map(|entry| entry.section.id));
    if let Some(taken) = first_taken(&children, &mut ids) {
        return Err(TreeError::DuplicateId(taken));
    }
    if let Some(section) = find_mut(tree, parent) {
        section.children.extend(children);
    }
    Ok(())
}

/// The broad-pass sections from section `at` of chapter `chapter` onwards,
/// across the chapters that follow.
fn broad_pass_from(
    tree: &IndexTree,
    chapter: usize,
    at: usize,
) -> impl Iterator<Item = &TreeSection> {
    let rest = tree.chapters.get(chapter + 1..).unwrap_or_default();
    tree.chapters
        .get(chapter)
        .and_then(|chapter| chapter.sections.get(at..))
        .unwrap_or_default()
        .iter()
        .chain(rest.iter().flat_map(|chapter| &chapter.sections))
}

/// Leaves in `path` the child positions leading to `id` and returns true, or
/// leaves it unchanged and returns false.
fn path_to(sections: &[TreeSection], id: Uuid, path: &mut Vec<usize>) -> bool {
    for (position, section) in sections.iter().enumerate() {
        path.push(position);
        if section.id == id || path_to(&section.children, id, path) {
            return true;
        }
        path.pop();
    }
    false
}

fn find(tree: &IndexTree, id: Uuid) -> Option<&TreeSection> {
    in_index_order(tree)
        .into_iter()
        .find(|entry| entry.section.id == id)
        .map(|entry| entry.section)
}

fn find_mut(tree: &mut IndexTree, id: Uuid) -> Option<&mut TreeSection> {
    fn walk(sections: &mut [TreeSection], id: Uuid) -> Option<&mut TreeSection> {
        sections.iter_mut().find_map(|section| {
            if section.id == id {
                Some(section)
            } else {
                walk(&mut section.children, id)
            }
        })
    }
    tree.chapters
        .iter_mut()
        .find_map(|chapter| walk(&mut chapter.sections, id))
}
