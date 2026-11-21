//! The build-index task: the broad pass and the course map in one structured
//! answer, its schema, its domain check and the input it is sent.

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::course_map::{self, FaultKind, MapFault};
use crate::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use crate::domain::course_map::{Concept, CourseMap, MapEntry};
use crate::domain::ladder::{Ladder, RungId};
use crate::domain::profile::{CandidateProfile, Knowledge};
use crate::harness::prompts::{self, Prompt};
use crate::harness::structured::StructuredTask;
use crate::index_tree::{IndexTree, TreeChapter, TreeSection, in_index_order};
use crate::protocol::Goal;
use crate::provider::{OutputSchema, Task};

/// The fewest chapters a broad pass has.
const MIN_CHAPTERS: usize = 2;

/// The index and its course map as the model writes them. The schema
/// requires every field; a section or a course with nothing to name answers
/// with empty lists.
#[derive(Debug, Deserialize)]
pub(super) struct IndexAnswer {
    chapters: Vec<ChapterAnswer>,
    concepts: Vec<ConceptAnswer>,
    conventions: Vec<ConventionAnswer>,
}

#[derive(Debug, Deserialize)]
struct ChapterAnswer {
    title: String,
    sections: Vec<SectionAnswer>,
}

#[derive(Debug, Deserialize)]
struct SectionAnswer {
    title: String,
    /// One line on what it covers.
    summary: String,
    introduces: Vec<String>,
    relies_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConceptAnswer {
    name: String,
    rung: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ConventionAnswer {
    kind: KindAnswer,
    key: String,
    meaning: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KindAnswer {
    Notation,
    Terminology,
    RunningExample,
}

impl From<KindAnswer> for ConventionKind {
    fn from(kind: KindAnswer) -> Self {
        match kind {
            KindAnswer::Notation => Self::Notation,
            KindAnswer::Terminology => Self::Terminology,
            KindAnswer::RunningExample => Self::RunningExample,
        }
    }
}

/// What a build stores: the tree with new ids, nothing read, its map and the
/// conventions the plan fixed.
pub(super) struct Plan {
    pub(super) tree: IndexTree,
    pub(super) map: CourseMap,
    pub(super) conventions: Vec<Convention>,
}

impl IndexAnswer {
    /// The answer as a tree with new ids and the map on those ids. Names and
    /// titles are trimmed; the concepts are the ones listed, each once, then
    /// any an entry names without listing it, with no rung.
    pub(super) fn plan(&self) -> Plan {
        let mut tree = IndexTree {
            chapters: Vec::with_capacity(self.chapters.len()),
        };
        let mut entries = Vec::new();
        for chapter in &self.chapters {
            let mut sections = Vec::with_capacity(chapter.sections.len());
            for section in &chapter.sections {
                let id = Uuid::new_v4();
                sections.push(TreeSection {
                    id,
                    title: section.title.trim().to_owned(),
                    line: section.summary.trim().to_owned(),
                    read: false,
                    children: Vec::new(),
                });
                entries.push(MapEntry {
                    section: id,
                    introduces: trimmed(&section.introduces),
                    relies_on: trimmed(&section.relies_on),
                });
            }
            tree.chapters.push(TreeChapter {
                id: Uuid::new_v4(),
                title: chapter.title.trim().to_owned(),
                sections,
            });
        }

        let mut concepts: Vec<Concept> = Vec::new();
        let listed = self.concepts.iter().map(|c| (c.name.trim(), c.rung));
        let named = entries
            .iter()
            .flat_map(|entry| entry.introduces.iter().chain(&entry.relies_on))
            .map(|name| (name.as_str(), None));
        for (name, rung) in listed.chain(named) {
            if !concepts.iter().any(|concept| concept.name == name) {
                concepts.push(Concept {
                    name: name.to_owned(),
                    rung: rung.map(RungId),
                });
            }
        }

        let conventions = self
            .conventions
            .iter()
            .map(|convention| Convention {
                kind: convention.kind.into(),
                key: convention.key.trim().to_owned(),
                meaning: convention.meaning.trim().to_owned(),
                origin: ConventionOrigin::Plan,
            })
            .collect();

        Plan {
            tree,
            map: CourseMap { concepts, entries },
            conventions,
        }
    }
}

fn trimmed(names: &[String]) -> Vec<String> {
    names.iter().map(|name| name.trim().to_owned()).collect()
}

/// The build-index task for a course with `ladder`, against which the map's
/// rungs are checked.
pub(super) struct BuildIndex<'a> {
    pub(super) ladder: &'a Ladder,
}

impl StructuredTask for BuildIndex<'_> {
    type Output = IndexAnswer;

    fn task(&self) -> Task {
        Task::BuildIndex
    }

    fn prompt(&self) -> Prompt {
        prompts::BUILD_INDEX
    }

    fn schema(&self) -> OutputSchema {
        let names = |description: &str| {
            json!({
                "type": "array",
                "description": description,
                "items": { "type": "string" }
            })
        };
        OutputSchema {
            name: "course_index".to_owned(),
            schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["chapters", "concepts", "conventions"],
                "properties": {
                    "chapters": {
                        "type": "array",
                        "description": "The broad pass, in reading order: at least two chapters.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["title", "sections"],
                            "properties": {
                                "title": { "type": "string" },
                                "sections": {
                                    "type": "array",
                                    "description": "At least one section, in reading order.",
                                    "items": {
                                        "type": "object",
                                        "additionalProperties": false,
                                        "required": ["title", "summary", "introduces", "relies_on"],
                                        "properties": {
                                            "title": { "type": "string" },
                                            "summary": {
                                                "type": "string",
                                                "description": "One line on what the section covers."
                                            },
                                            "introduces": names(
                                                "The concepts this section introduces, by name."
                                            ),
                                            "relies_on": names(
                                                "The concepts this section relies on, by name, \
                                                 each introduced by an earlier section."
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "concepts": {
                        "type": "array",
                        "description": "Every concept the sections name, each once.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["name", "rung"],
                            "properties": {
                                "name": { "type": "string" },
                                "rung": {
                                    "type": ["integer", "null"],
                                    "description": "The id of the ladder rung the concept \
                                                    belongs to; null when it has none."
                                }
                            }
                        }
                    },
                    "conventions": {
                        "type": "array",
                        "description": "The notation, terminology and running examples the \
                                        whole course keeps.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["kind", "key", "meaning"],
                            "properties": {
                                "kind": {
                                    "type": "string",
                                    "enum": ["notation", "terminology", "running_example"]
                                },
                                "key": {
                                    "type": "string",
                                    "description": "The symbol, the term or the example's name."
                                },
                                "meaning": { "type": "string" }
                            }
                        }
                    }
                }
            }),
        }
    }

    fn check(&self, answer: &IndexAnswer) -> Result<(), Vec<String>> {
        let mut reasons = Vec::new();
        let count = answer.chapters.len();
        if count < MIN_CHAPTERS {
            reasons.push(format!(
                "the index has {count} chapters; it needs at least {MIN_CHAPTERS}"
            ));
        }
        for (number, chapter) in (1..).zip(&answer.chapters) {
            let title = chapter.title.trim();
            if title.is_empty() {
                reasons.push(format!("chapter {number} has an empty title"));
            }
            if chapter.sections.is_empty() {
                reasons.push(format!("chapter {number}, \"{title}\", has no sections"));
            }
            for (at, section) in (1..).zip(&chapter.sections) {
                let section_title = section.title.trim();
                if section_title.is_empty() {
                    reasons.push(format!(
                        "section {at} of chapter {number}, \"{title}\", has an empty title"
                    ));
                }
                if section.summary.trim().is_empty() {
                    reasons.push(format!(
                        "section {at} of chapter {number}, \"{section_title}\", has an empty line"
                    ));
                }
            }
        }

        let plan = answer.plan();
        let order: Vec<Uuid> = in_index_order(&plan.tree)
            .iter()
            .map(|entry| entry.section.id)
            .collect();
        let titles: HashMap<Uuid, &str> = in_index_order(&plan.tree)
            .iter()
            .map(|entry| (entry.section.id, entry.section.title.as_str()))
            .collect();
        for fault in course_map::validate(&plan.map, &order, self.ladder) {
            let title = titles.get(&fault.section).copied().unwrap_or_default();
            reasons.push(describe(&fault, title));
        }

        if reasons.is_empty() {
            Ok(())
        } else {
            Err(reasons)
        }
    }
}

/// A map fault, naming the section by its title for the repair request.
fn describe(fault: &MapFault, section: &str) -> String {
    let concept = fault.concept.as_deref().unwrap_or_default();
    match fault.kind {
        FaultKind::NotIntroducedEarlier => format!(
            "section \"{section}\" relies on \"{concept}\", which no earlier section introduces"
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

/// The task's input: the topic, the goal, every rung of the ladder with its
/// id, concept and state, and the misconceptions picked. Without a profile
/// every rung is unknown.
pub(super) fn input(
    topic: &str,
    goal: Goal,
    ladder: &Ladder,
    profile: Option<&CandidateProfile>,
) -> String {
    let goal = match goal {
        Goal::Overview => "overview",
        Goal::WorkingKnowledge => "working knowledge",
        Goal::Expert => "expert",
    };
    let mut input = format!("Topic: {topic}\nGoal: {goal}\n\nLadder, most basic first:\n");
    for rung in &ladder.rungs {
        let state = profile
            .and_then(|profile| profile.rungs.iter().find(|known| known.rung == rung.id))
            .map_or(Knowledge::Unknown, |known| known.state);
        let state = match state {
            Knowledge::Known => "known",
            Knowledge::Shaky => "shaky",
            Knowledge::Unknown => "unknown",
        };
        let _ = writeln!(input, "- rung {} ({state}): {}", rung.id.0, rung.concept);
    }
    let misconceptions = profile.map_or(&[][..], |profile| &profile.misconceptions);
    if !misconceptions.is_empty() {
        input.push_str("\nMisconceptions picked:\n");
        for misconception in misconceptions {
            let _ = writeln!(
                input,
                "- rung {}: {}",
                misconception.rung.0, misconception.text
            );
        }
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ladder::Rung;
    use crate::harness::structured::strict_faults;

    #[test]
    fn without_a_profile_every_rung_is_unknown() {
        let ladder = Ladder {
            rungs: vec![
                Rung {
                    id: RungId(1),
                    concept: "The vocabulary".to_owned(),
                },
                Rung {
                    id: RungId(2),
                    concept: "The core rule".to_owned(),
                },
            ],
        };

        let input = input("Plate tectonics", Goal::Overview, &ladder, None);

        assert!(
            input.contains("- rung 1 (unknown): The vocabulary"),
            "{input}"
        );
        assert!(
            input.contains("- rung 2 (unknown): The core rule"),
            "{input}"
        );
        assert!(!input.contains("Misconceptions"), "{input}");
    }

    #[test]
    fn the_schema_is_one_the_codex_backend_accepts() {
        let ladder = Ladder { rungs: Vec::new() };

        let faults = strict_faults(&BuildIndex { ladder: &ladder }.schema().schema);

        assert!(faults.is_empty(), "{faults:#?}");
    }
}
