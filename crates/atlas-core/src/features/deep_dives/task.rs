//! The propose-deep-dives task: three to six children of a section with their
//! part of the course map in one structured answer, its schema, its domain
//! check and the input it is sent.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::course_map::{self, FaultKind, MapFault};
use crate::domain::conventions::Convention;
use crate::domain::course_map::{Concept, CourseMap, MapEntry};
use crate::domain::ladder::{Ladder, RungId};
use crate::domain::profile::CandidateProfile;
use crate::domain::record::StoredRecord;
use crate::harness::context::{
    IndexKeys, render_conventions, render_index, render_profile, render_record,
};
use crate::harness::prompts::{self, Prompt};
use crate::harness::structured::StructuredTask;
use crate::index_tree::{DEEP_DIVE_SIZE, IndexTree, TreeSection, in_index_order};
use crate::provider::{OutputSchema, Task};

/// The children as the model writes them.
#[derive(Debug, Deserialize)]
pub(super) struct ProposalsAnswer {
    children: Vec<ChildAnswer>,
}

#[derive(Debug, Deserialize)]
struct ChildAnswer {
    title: String,
    /// One line on what it covers.
    summary: String,
    introduces: Vec<ConceptAnswer>,
    relies_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConceptAnswer {
    name: String,
    rung: Option<u32>,
}

/// What a deep dive stores: the children with new ids, nothing read, and
/// their part of the map: the concepts they introduce, each once, and an
/// entry per child.
pub(super) struct Proposals {
    pub(super) children: Vec<TreeSection>,
    pub(super) map: CourseMap,
}

impl ProposalsAnswer {
    /// The answer as children with new ids and their map on those ids. Names
    /// and titles are trimmed; a concept introduced twice keeps its first
    /// rung.
    pub(super) fn proposals(&self) -> Proposals {
        let mut children = Vec::with_capacity(self.children.len());
        let mut concepts: Vec<Concept> = Vec::new();
        let mut entries = Vec::with_capacity(self.children.len());
        for child in &self.children {
            let id = Uuid::new_v4();
            children.push(TreeSection {
                id,
                title: child.title.trim().to_owned(),
                line: child.summary.trim().to_owned(),
                read: false,
                children: Vec::new(),
            });
            for concept in &child.introduces {
                let name = concept.name.trim();
                if !concepts.iter().any(|known| known.name == name) {
                    concepts.push(Concept {
                        name: name.to_owned(),
                        rung: concept.rung.map(RungId),
                    });
                }
            }
            entries.push(MapEntry {
                section: id,
                introduces: child
                    .introduces
                    .iter()
                    .map(|concept| concept.name.trim().to_owned())
                    .collect(),
                relies_on: child
                    .relies_on
                    .iter()
                    .map(|name| name.trim().to_owned())
                    .collect(),
            });
        }
        Proposals {
            children,
            map: CourseMap { concepts, entries },
        }
    }
}

/// The propose-deep-dives task for `parent`, a section of `tree` without
/// children, checked against the course's map and ladder.
pub(super) struct ProposeDeepDives<'a> {
    pub(super) parent: Uuid,
    pub(super) tree: &'a IndexTree,
    pub(super) map: &'a CourseMap,
    pub(super) ladder: &'a Ladder,
}

impl ProposeDeepDives<'_> {
    /// The course map with the proposals' entries and new concepts added, or
    /// every fault of the proposals, read with the children in place under
    /// the parent.
    pub(super) fn extend(&self, proposals: &Proposals) -> Result<CourseMap, Vec<MapFault>> {
        let order = self.order_with(&proposals.children);
        course_map::extend(self.map, &order, &proposals.map, self.ladder)
    }

    /// The index order once `children` follow the parent's own sections.
    fn order_with(&self, children: &[TreeSection]) -> Vec<Uuid> {
        let order = in_index_order(self.tree);
        let at = order
            .iter()
            .position(|entry| entry.section.id == self.parent)
            .map_or(order.len(), |at| {
                let depth = order[at].depth;
                at + 1
                    + order[at + 1..]
                        .iter()
                        .take_while(|entry| entry.depth > depth)
                        .count()
            });
        let mut ids: Vec<Uuid> = order.iter().map(|entry| entry.section.id).collect();
        ids.splice(at..at, children.iter().map(|child| child.id));
        ids
    }
}

impl StructuredTask for ProposeDeepDives<'_> {
    type Output = ProposalsAnswer;

    fn task(&self) -> Task {
        Task::ProposeDeepDives
    }

    fn prompt(&self) -> Prompt {
        prompts::PROPOSE_DEEP_DIVES
    }

    fn schema(&self) -> OutputSchema {
        OutputSchema {
            name: "deep_dive".to_owned(),
            schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["children"],
                "properties": {
                    "children": {
                        "type": "array",
                        "description": "Three to six child sections, in reading order.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["title", "summary", "introduces", "relies_on"],
                            "properties": {
                                "title": {
                                    "type": "string",
                                    "description": "Distinct from its siblings' titles."
                                },
                                "summary": {
                                    "type": "string",
                                    "description": "One line on what the section covers."
                                },
                                "introduces": {
                                    "type": "array",
                                    "description": "The concepts this section introduces.",
                                    "items": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["name", "rung"],
                                        "properties": {
                                            "name": { "type": "string" },
                                            "rung": {
                                                "type": ["integer", "null"],
                                                "description": "The id of the ladder rung the \
                                                                concept belongs to; null when \
                                                                it has none."
                                            }
                                        }
                                    }
                                },
                                "relies_on": {
                                    "type": "array",
                                    "description": "The concepts this section relies on, by \
                                                    name, each introduced up to the parent \
                                                    section or by an earlier sibling.",
                                    "items": { "type": "string" }
                                }
                            }
                        }
                    }
                }
            }),
        }
    }

    fn check(&self, answer: &ProposalsAnswer) -> Result<(), Vec<String>> {
        let mut reasons = Vec::new();
        let count = answer.children.len();
        if !DEEP_DIVE_SIZE.contains(&count) {
            reasons.push(format!(
                "the deep dive has {count} sections; it needs {} to {}",
                DEEP_DIVE_SIZE.start(),
                DEEP_DIVE_SIZE.end()
            ));
        }
        let mut titles = HashSet::new();
        for (number, child) in (1..).zip(&answer.children) {
            let title = child.title.trim();
            if title.is_empty() {
                reasons.push(format!("section {number} has an empty title"));
            } else if !titles.insert(title.to_lowercase()) {
                reasons.push(format!(
                    "section {number} repeats the title \"{title}\" of an earlier sibling"
                ));
            }
            if child.summary.trim().is_empty() {
                reasons.push(format!("section {number}, \"{title}\", has an empty line"));
            }
        }

        let proposals = answer.proposals();
        if let Err(faults) = self.extend(&proposals) {
            let titles: HashMap<Uuid, &str> = proposals
                .children
                .iter()
                .map(|child| (child.id, child.title.as_str()))
                .collect();
            reasons.extend(
                faults
                    .iter()
                    .map(|fault| describe(fault, titles.get(&fault.section).copied())),
            );
        }

        if reasons.is_empty() {
            Ok(())
        } else {
            Err(reasons)
        }
    }
}

/// A map fault, naming a proposed section by its title for the repair
/// request.
fn describe(fault: &MapFault, title: Option<&str>) -> String {
    let Some(section) = title else {
        return fault.to_string();
    };
    let concept = fault.concept.as_deref().unwrap_or_default();
    match fault.kind {
        FaultKind::NotIntroducedEarlier => format!(
            "section \"{section}\" relies on \"{concept}\", which neither the course up to the \
             parent section nor an earlier sibling introduces"
        ),
        FaultKind::UnknownRung => format!(
            "section \"{section}\" introduces \"{concept}\", whose rung is not on the ladder"
        ),
        FaultKind::EmptyName => {
            format!("section \"{section}\" names a concept with an empty name")
        }
        FaultKind::MissingEntry | FaultKind::UnknownSection | FaultKind::DuplicateEntry => {
            format!("section \"{section}\": {fault}")
        }
    }
}

/// What the task's input is built from.
pub(super) struct Course<'a> {
    pub(super) ladder: &'a Ladder,
    pub(super) profile: &'a CandidateProfile,
    pub(super) tree: &'a IndexTree,
    pub(super) map: &'a CourseMap,
    pub(super) conventions: &'a [Convention],
}

/// The task's input: the profile, the index titles with their keys, the
/// conventions, the parent section with its map entry and its record, then
/// the concepts the course introduces up to it, which the children may rely
/// on, and those it introduces later, each with its rung.
pub(super) fn input(
    course: &Course<'_>,
    parent: &TreeSection,
    record: Option<&StoredRecord>,
) -> String {
    let mut input = format!(
        "# Candidate profile\n{}\n\n# Index\n{}\n\n{}\n\n",
        render_profile(course.ladder, course.profile),
        render_index(course.tree),
        render_conventions(course.conventions),
    );

    let keys = IndexKeys::new(course.tree);
    let entry = course_map::entry(course.map, parent.id);
    let list = |names: Option<&Vec<String>>, none: &str| match names {
        Some(names) if !names.is_empty() => names.join(", "),
        _ => none.to_owned(),
    };
    let _ = write!(
        input,
        "# The section to go deeper into: {} {}\nCovers: {}\nIntroduces: {}\nRelies on: {}\n\
         Its record:\n{}",
        keys.key(parent.id).unwrap_or_default(),
        parent.title,
        parent.line,
        list(entry.map(|entry| &entry.introduces), "nothing planned"),
        list(entry.map(|entry| &entry.relies_on), "nothing"),
        record.map_or_else(
            || "None stored.".to_owned(),
            |stored| render_record(&stored.record, stored.status)
        ),
    );

    // Where each concept is first introduced, in index order.
    let position: HashMap<Uuid, usize> = in_index_order(course.tree)
        .iter()
        .enumerate()
        .map(|(at, entry)| (entry.section.id, at))
        .collect();
    let mut first: HashMap<&str, usize> = HashMap::new();
    for entry in &course.map.entries {
        let Some(&at) = position.get(&entry.section) else {
            continue;
        };
        for name in &entry.introduces {
            first
                .entry(name.as_str())
                .and_modify(|earliest| *earliest = (*earliest).min(at))
                .or_insert(at);
        }
    }
    let parent_at = position.get(&parent.id).copied();
    let mut earlier = String::new();
    let mut later = String::new();
    for concept in &course.map.concepts {
        let Some(&at) = first.get(concept.name.as_str()) else {
            continue;
        };
        let list = if parent_at.is_some_and(|parent_at| at <= parent_at) {
            &mut earlier
        } else {
            &mut later
        };
        let _ = write!(list, "\n- {}", concept.name);
        if let Some(rung) = concept.rung {
            let _ = write!(list, " (rung {})", rung.0);
        }
    }
    let none = |list: String| {
        if list.is_empty() {
            "\nNone.".to_owned()
        } else {
            list
        }
    };
    let _ = write!(
        input,
        "\n\n# Concepts the course introduces up to this section, which the children may \
         rely on{}\n\n# Concepts the course introduces later, which the children may not rely \
         on{}",
        none(earlier),
        none(later),
    );
    input
}
