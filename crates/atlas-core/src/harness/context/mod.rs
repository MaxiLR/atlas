//! The context a section is written from, chosen by the core with no request
//! (ADR-0013), and its fixed shrink order over budget.
//!
//! The renderings of the candidate profile and of the index titles with their
//! keys are shared with the other tasks that send them.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use uuid::Uuid;

use crate::course_map;
use crate::domain::conventions::{Convention, ConventionKind};
use crate::domain::course_map::{CourseMap, MapEntry};
use crate::domain::ladder::Ladder;
use crate::domain::profile::{CandidateProfile, Knowledge};
use crate::domain::record::{Promise, RecordStatus, SectionRecord, StoredRecord};
use crate::index_tree::{IndexTree, TreeSection, in_index_order};
use crate::protocol::Goal;

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
/// characters: the whole record as JSON. The section budget is sized from
/// them.
pub const RECORD_CHARS: usize = 4_000;
/// The record's summary.
pub const RECORD_SUMMARY_CHARS: usize = 600;
/// One concept line, meaning, example or promise.
pub const RECORD_LINE_CHARS: usize = 300;
/// The entries of each of the record's lists.
pub const RECORD_ENTRIES: usize = 12;

/// The candidate profile as a request carries it: the goal, every rung of the
/// ladder with its concept and state, and the misconceptions picked. A rung
/// the profile does not mention is unknown.
pub fn render_profile(ladder: &Ladder, profile: &CandidateProfile) -> String {
    let goal = match profile.goal {
        Goal::Overview => "overview",
        Goal::WorkingKnowledge => "working knowledge",
        Goal::Expert => "expert",
    };
    let mut text = format!("Goal: {goal}\nLadder, most basic first:");
    for rung in &ladder.rungs {
        let state = profile
            .rungs
            .iter()
            .find(|known| known.rung == rung.id)
            .map_or(Knowledge::Unknown, |known| known.state);
        let state = match state {
            Knowledge::Known => "known",
            Knowledge::Shaky => "shaky",
            Knowledge::Unknown => "unknown",
        };
        let _ = write!(text, "\n- rung {}, {}: {state}", rung.id.0, rung.concept);
    }
    if !profile.misconceptions.is_empty() {
        text.push_str("\nMisconceptions picked:");
        for misconception in &profile.misconceptions {
            let _ = write!(
                text,
                "\n- rung {}: {}",
                misconception.rung.0, misconception.text
            );
        }
    }
    text
}

/// The short stable key of each index entry: `2` for a chapter, `2.3` for a
/// section and `2.3.1` for a deep dive under it. Deep dives are added after
/// existing children, so a key never changes once given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexKeys {
    by_node: HashMap<Uuid, String>,
    by_key: HashMap<String, Uuid>,
}

impl IndexKeys {
    pub fn new(tree: &IndexTree) -> Self {
        let mut keys = Self::default();
        walk(tree, |key, _, node, _| {
            keys.by_node.insert(node, key.to_owned());
            keys.by_key.insert(key.to_owned(), node);
        });
        keys
    }

    pub fn key(&self, node: Uuid) -> Option<&str> {
        self.by_node.get(&node).map(String::as_str)
    }

    pub fn node(&self, key: &str) -> Option<Uuid> {
        self.by_key.get(key).copied()
    }

    /// `2.3 Title`.
    fn label(&self, node: Uuid, title: &str) -> String {
        format!("{} {title}", self.key(node).unwrap_or_default())
    }
}

/// The index titles with their keys, one per line, indented by their place
/// in the tree.
pub fn render_index(tree: &IndexTree) -> String {
    let mut lines = Vec::new();
    walk(tree, |key, depth, _, title| {
        lines.push(format!("{:indent$}{key} {title}", "", indent = depth * 2));
    });
    lines.join("\n")
}

/// The chapters and sections in index order, each with its key, its depth (0
/// for a chapter, 1 for the broad pass, 2 for a deep dive under it and so on),
/// its id and its title.
fn walk(tree: &IndexTree, mut visit: impl FnMut(&str, usize, Uuid, &str)) {
    fn sections(
        prefix: &str,
        depth: usize,
        list: &[TreeSection],
        visit: &mut impl FnMut(&str, usize, Uuid, &str),
    ) {
        for (at, section) in list.iter().enumerate() {
            let key = format!("{prefix}.{}", at + 1);
            visit(&key, depth, section.id, &section.title);
            sections(&key, depth + 1, &section.children, visit);
        }
    }
    for (at, chapter) in tree.chapters.iter().enumerate() {
        let key = (at + 1).to_string();
        visit(&key, 0, chapter.id, &chapter.title);
        sections(&key, 1, &chapter.sections, &mut visit);
    }
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

impl Shrink {
    const ORDER: [Shrink; 3] = [
        Shrink::ChapterTitles,
        Shrink::PreviousSummary,
        Shrink::PrerequisiteLines,
    ];

    fn part(self) -> PartKind {
        match self {
            Shrink::ChapterTitles => PartKind::OtherRead,
            Shrink::PreviousSummary => PartKind::Previous,
            Shrink::PrerequisiteLines => PartKind::Prerequisites,
        }
    }
}

/// The input of a section request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionContext {
    pub parts: Vec<ContextPart>,
    /// The steps applied to fit the budget, in order.
    pub shrunk: Vec<Shrink>,
}

/// Between two parts of the input.
const PART_SEPARATOR: &str = "\n\n";

impl SectionContext {
    /// The request's input: the parts in order.
    pub fn input(&self) -> String {
        self.parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join(PART_SEPARATOR)
    }

    pub fn part(&self, kind: PartKind) -> Option<&str> {
        self.parts
            .iter()
            .find(|part| part.kind == kind)
            .map(|part| part.text.as_str())
    }

    /// The input's length in characters.
    fn chars(&self) -> usize {
        let separators = self.parts.len().saturating_sub(1) * PART_SEPARATOR.chars().count();
        self.parts
            .iter()
            .map(|part| part.text.chars().count())
            .sum::<usize>()
            + separators
    }
}

/// The context `node` is written from: the profile, the index titles, the
/// conventions, its map entry, the records of the sections that introduced
/// what it relies on, the previous section's record, the promises due to it,
/// a deep dive's parent's record and one line for every other section read.
/// Only the records of sections read are sent, each once: a parent's or a
/// previous section's record is not repeated among the prerequisites. Over
/// `budget` the parts shrink in the order of [`Shrink`], stopping as soon as
/// the input fits. None when `node` is not a section of the index.
pub fn section_context(
    memory: &CourseMemory,
    node: Uuid,
    budget: Budget,
) -> Option<SectionContext> {
    let selection = Selection::new(memory, node)?;
    let mut context = SectionContext {
        parts: selection.parts(),
        shrunk: Vec::new(),
    };
    let mut size = context.chars();
    for step in Shrink::ORDER {
        if size <= budget.0 {
            break;
        }
        context.shrunk.push(step);
        let kind = step.part();
        if let Some(part) = context.parts.iter_mut().find(|part| part.kind == kind) {
            let text = selection.shrunk(kind);
            size = size - part.text.chars().count() + text.chars().count();
            part.text = text;
        }
    }
    Some(context)
}

/// A section read and the record of its current version.
#[derive(Clone, Copy)]
struct Recorded<'a> {
    section: &'a TreeSection,
    record: &'a StoredRecord,
}

/// A read section that introduced concepts the section relies on.
struct Introducer<'a> {
    section: &'a TreeSection,
    record: Option<&'a StoredRecord>,
    /// The concepts relied on that it introduced.
    concepts: Vec<&'a str>,
    /// Where its whole record is sent instead, when it is the parent or the
    /// previous section.
    sent_in: Option<&'static str>,
}

/// A concept relied on whose introducing section the candidate has not read,
/// or which no section introduces.
struct NotYetRead<'a> {
    concept: &'a str,
    introducer: Option<&'a TreeSection>,
}

/// What the core selected for one section.
struct Selection<'a> {
    memory: &'a CourseMemory,
    keys: IndexKeys,
    target: &'a TreeSection,
    entry: Option<&'a MapEntry>,
    /// In index order.
    introducers: Vec<Introducer<'a>>,
    not_yet_read: Vec<NotYetRead<'a>>,
    previous: Option<Recorded<'a>>,
    promises: Vec<(&'a TreeSection, &'a Promise)>,
    parent: Option<Recorded<'a>>,
    /// Each other section read with its chapter's position, in index order.
    other_read: Vec<(usize, &'a TreeSection)>,
}

impl<'a> Selection<'a> {
    fn new(memory: &'a CourseMemory, node: Uuid) -> Option<Self> {
        let order = in_index_order(&memory.index);
        let at = order.iter().position(|entry| entry.section.id == node)?;
        let target = order[at];
        let keys = IndexKeys::new(&memory.index);
        let ids: Vec<Uuid> = order.iter().map(|entry| entry.section.id).collect();
        let recorded = |at: usize| {
            let section = order[at].section;
            let record = memory.records.get(&section.id).filter(|_| section.read)?;
            Some(Recorded { section, record })
        };

        let parent_at = order[..at]
            .iter()
            .rposition(|entry| entry.depth + 1 == target.depth)
            .filter(|_| target.depth > 0);
        let parent = parent_at.and_then(recorded);
        let previous = at
            .checked_sub(1)
            .filter(|&before| Some(before) != parent_at)
            .and_then(recorded);

        let entry = course_map::entry(&memory.map, node);
        let relies_on = entry.map_or(&[][..], |entry| &entry.relies_on);
        let mut introducers: Vec<(usize, Introducer<'a>)> = Vec::new();
        let mut not_yet_read = Vec::new();
        let mut seen = HashSet::new();
        for concept in relies_on.iter().map(String::as_str) {
            if !seen.insert(concept) {
                continue;
            }
            let introducer = course_map::introducer(&memory.map, &ids, concept)
                .and_then(|id| ids.iter().position(|&candidate| candidate == id));
            let Some(introducer_at) = introducer.filter(|&at| order[at].section.read) else {
                not_yet_read.push(NotYetRead {
                    concept,
                    introducer: introducer.map(|at| order[at].section),
                });
                continue;
            };
            if let Some((_, known)) = introducers
                .iter_mut()
                .find(|(known_at, _)| *known_at == introducer_at)
            {
                known.concepts.push(concept);
                continue;
            }
            let section = order[introducer_at].section;
            let sent_in = if parent.is_some() && Some(introducer_at) == parent_at {
                Some("the parent section")
            } else if previous.is_some_and(|previous| previous.section.id == section.id) {
                Some("the previous section")
            } else {
                None
            };
            introducers.push((
                introducer_at,
                Introducer {
                    section,
                    record: memory.records.get(&section.id),
                    concepts: vec![concept],
                    sent_in,
                },
            ));
        }
        introducers.sort_by_key(|(at, _)| *at);

        let target_key = keys.key(node).unwrap_or_default();
        let promises = order[..at]
            .iter()
            .filter(|entry| entry.section.read)
            .filter_map(|entry| Some((entry.section, memory.records.get(&entry.section.id)?)))
            .flat_map(|(section, stored)| {
                stored
                    .record
                    .promises
                    .iter()
                    .map(move |promise| (section, promise))
            })
            .filter(|(_, promise)| promise.section == target_key)
            .collect();

        let mut sent: HashSet<Uuid> = introducers
            .iter()
            .map(|(_, introducer)| introducer.section.id)
            .chain(previous.iter().chain(&parent).map(|sent| sent.section.id))
            .collect();
        sent.insert(node);
        let other_read = order
            .iter()
            .filter(|entry| entry.section.read && !sent.contains(&entry.section.id))
            .filter_map(|entry| {
                let chapter = memory
                    .index
                    .chapters
                    .iter()
                    .position(|chapter| chapter.id == entry.chapter)?;
                Some((chapter, entry.section))
            })
            .collect();

        Some(Self {
            memory,
            keys,
            target: target.section,
            entry,
            introducers: introducers
                .into_iter()
                .map(|(_, introducer)| introducer)
                .collect(),
            not_yet_read,
            previous,
            promises,
            parent,
            other_read,
        })
    }

    /// Every part at full size, in order; parts with nothing to say are left
    /// out, except the profile, the index, the conventions and the entry.
    fn parts(&self) -> Vec<ContextPart> {
        let mut parts = vec![
            ContextPart {
                kind: PartKind::Profile,
                text: format!(
                    "# Candidate profile\n{}",
                    render_profile(&self.memory.ladder, &self.memory.profile)
                ),
            },
            ContextPart {
                kind: PartKind::Index,
                text: format!("# Index\n{}", render_index(&self.memory.index)),
            },
            ContextPart {
                kind: PartKind::Conventions,
                text: self.conventions(),
            },
            ContextPart {
                kind: PartKind::Entry,
                text: self.entry(),
            },
        ];
        if !self.introducers.is_empty() || !self.not_yet_read.is_empty() {
            parts.push(ContextPart {
                kind: PartKind::Prerequisites,
                text: self.prerequisites(false),
            });
        }
        if self.previous.is_some() {
            parts.push(ContextPart {
                kind: PartKind::Previous,
                text: self.previous(false),
            });
        }
        if !self.promises.is_empty() {
            parts.push(ContextPart {
                kind: PartKind::Promises,
                text: self.promises(),
            });
        }
        if let Some(parent) = self.parent {
            parts.push(ContextPart {
                kind: PartKind::Parent,
                text: format!(
                    "# Parent section: {}\n{}",
                    self.label(parent.section),
                    render_record(&parent.record.record, parent.record.status)
                ),
            });
        }
        if !self.other_read.is_empty() {
            parts.push(ContextPart {
                kind: PartKind::OtherRead,
                text: self.other_read(false),
            });
        }
        parts
    }

    /// The part `kind` once its shrinking step is applied.
    fn shrunk(&self, kind: PartKind) -> String {
        match kind {
            PartKind::OtherRead => self.other_read(true),
            PartKind::Previous => self.previous(true),
            PartKind::Prerequisites => self.prerequisites(true),
            PartKind::Profile
            | PartKind::Index
            | PartKind::Conventions
            | PartKind::Entry
            | PartKind::Promises
            | PartKind::Parent => unreachable!("{kind:?} never shrinks"),
        }
    }

    fn label(&self, section: &TreeSection) -> String {
        self.keys.label(section.id, &section.title)
    }

    fn conventions(&self) -> String {
        let mut text = "# Conventions, fixed for the whole course".to_owned();
        if self.memory.conventions.is_empty() {
            text.push_str("\nNone fixed yet.");
        }
        for convention in &self.memory.conventions {
            let kind = match convention.kind {
                ConventionKind::Notation => "symbol",
                ConventionKind::Terminology => "term",
                ConventionKind::RunningExample => "running example",
            };
            let _ = write!(
                text,
                "\n- {kind} \"{}\": {}",
                convention.key, convention.meaning
            );
        }
        text
    }

    fn entry(&self) -> String {
        let list = |concepts: Option<&Vec<String>>, none: &str| match concepts {
            Some(concepts) if !concepts.is_empty() => concepts.join(", "),
            _ => none.to_owned(),
        };
        format!(
            "# This section: {}\nCovers: {}\nIntroduces: {}\nRelies on: {}",
            self.label(self.target),
            self.target.line,
            list(self.entry.map(|entry| &entry.introduces), "nothing planned"),
            list(self.entry.map(|entry| &entry.relies_on), "nothing"),
        )
    }

    /// Each read introducer with its whole record, or only the lines of the
    /// concepts relied on; then the concepts not yet read.
    fn prerequisites(&self, lines_only: bool) -> String {
        let mut text = "# Concepts this section relies on".to_owned();
        for introducer in &self.introducers {
            let _ = write!(
                text,
                "\n## {}, which introduced: {}",
                self.label(introducer.section),
                introducer.concepts.join(", ")
            );
            if let Some(part) = introducer.sent_in {
                let _ = write!(text, " (its record is under {part})");
            }
            text.push('\n');
            match introducer.record {
                Some(stored) if !lines_only && introducer.sent_in.is_none() => {
                    text.push_str(&render_record(&stored.record, stored.status));
                }
                record => text.push_str(&concept_lines(
                    record.map(|stored| &stored.record),
                    &introducer.concepts,
                )),
            }
        }
        if !self.not_yet_read.is_empty() {
            text.push_str("\n## Not yet read by the candidate: recap these");
            for missing in &self.not_yet_read {
                let from = missing.introducer.map_or_else(
                    || "introduced by no section".to_owned(),
                    |section| format!("introduced in {}", self.label(section)),
                );
                let _ = write!(text, "\n- {}: not yet read, {from}", missing.concept);
            }
        }
        text
    }

    fn previous(&self, summary_only: bool) -> String {
        let Some(previous) = self.previous else {
            return String::new();
        };
        let stored = previous.record;
        let record = match &stored.record.summary {
            Some(summary) if summary_only => format!("Summary: {summary}"),
            _ => render_record(&stored.record, stored.status),
        };
        format!(
            "# Previous section: {}\n{record}",
            self.label(previous.section)
        )
    }

    fn promises(&self) -> String {
        let mut text = "# Promised to this section by earlier sections".to_owned();
        for (section, promise) in &self.promises {
            let _ = write!(text, "\n- {}: {}", self.label(section), promise.what);
        }
        text
    }

    /// One line per other section read under its chapter's title, or the
    /// chapters' titles only.
    fn other_read(&self, titles_only: bool) -> String {
        let mut text = if titles_only {
            "# Chapters with other sections read".to_owned()
        } else {
            "# Other sections read".to_owned()
        };
        let mut chapter_shown = None;
        for &(chapter, section) in &self.other_read {
            if chapter_shown != Some(chapter) {
                chapter_shown = Some(chapter);
                let title = &self.memory.index.chapters[chapter].title;
                let _ = write!(text, "\n{} {title}", chapter + 1);
            }
            if !titles_only {
                let _ = write!(text, "\n- {}: {}", self.label(section), section.line);
            }
        }
        text
    }
}

/// A record whole, but for its promises: those due to the section are sent
/// apart. An unconfirmed record says so.
fn render_record(record: &SectionRecord, status: RecordStatus) -> String {
    let mut lines = Vec::new();
    let taught = match status {
        RecordStatus::Confirmed => "Concepts taught:",
        RecordStatus::Predicted => {
            lines.push("Record unconfirmed: predicted from the plan.".to_owned());
            "Concepts planned:"
        }
    };
    if let Some(summary) = &record.summary {
        lines.push(format!("Summary: {summary}"));
    }
    if !record.concepts.is_empty() {
        lines.push(taught.to_owned());
        for concept in &record.concepts {
            lines.push(match &concept.line {
                Some(line) => format!("- {}: {line}", concept.name),
                None => format!("- {}", concept.name),
            });
        }
    }
    for (title, entries) in [
        ("Notation fixed:", &record.notation),
        ("Terms fixed:", &record.terms),
    ] {
        if !entries.is_empty() {
            lines.push(title.to_owned());
            lines.extend(
                entries
                    .iter()
                    .map(|entry| format!("- {}: {}", entry.key, entry.meaning)),
            );
        }
    }
    if !record.examples.is_empty() {
        lines.push("Examples used:".to_owned());
        lines.extend(record.examples.iter().map(|example| format!("- {example}")));
    }
    lines.join("\n")
}

/// Each concept with its line as the record words it, or its name alone.
fn concept_lines(record: Option<&SectionRecord>, concepts: &[&str]) -> String {
    concepts
        .iter()
        .map(|&name| {
            let line = record
                .and_then(|record| record.concepts.iter().find(|taught| taught.name == name))
                .and_then(|taught| taught.line.as_deref());
            match line {
                Some(line) => format!("- {name}: {line}"),
                None => format!("- {name}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
