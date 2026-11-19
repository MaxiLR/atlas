//! The two structured tasks of placement: draft ladder and placement question,
//! each with its schema, its domain check and the input it is sent.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::ladder::{Ladder, Rung, RungId};
use crate::domain::questions::{KeyedOption, KeyedPlacementQuestion};
use crate::harness::prompts::{self, Prompt};
use crate::harness::structured::StructuredTask;
use crate::protocol::Goal;
use crate::provider::{OutputSchema, Task};

/// The fewest and the most concepts a ladder has.
const MIN_CONCEPTS: usize = 8;
const MAX_CONCEPTS: usize = 12;

/// A placement question has four options; "Not sure" is the app's.
const OPTIONS: usize = 4;

/// The ladder as the model writes it: concepts in prerequisite order.
#[derive(Debug, Deserialize)]
pub(super) struct LadderDraft {
    concepts: Vec<String>,
}

impl LadderDraft {
    /// The ladder, rungs numbered from 1 at the most basic.
    pub(super) fn into_ladder(self) -> Ladder {
        Ladder {
            rungs: (1..)
                .zip(self.concepts)
                .map(|(id, concept)| Rung {
                    id: RungId(id),
                    concept: concept.trim().to_owned(),
                })
                .collect(),
        }
    }
}

pub(super) struct DraftLadder;

impl StructuredTask for DraftLadder {
    type Output = LadderDraft;

    fn task(&self) -> Task {
        Task::DraftLadder
    }

    fn prompt(&self) -> Prompt {
        prompts::DRAFT_LADDER
    }

    fn schema(&self) -> OutputSchema {
        OutputSchema {
            name: "concept_ladder".to_owned(),
            schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["concepts"],
                "properties": {
                    "concepts": {
                        "type": "array",
                        "description": "Eight to twelve concepts of the topic in prerequisite \
                                        order, the most basic first, each named once, in English.",
                        "items": { "type": "string" }
                    }
                }
            }),
        }
    }

    fn check(&self, ladder: &LadderDraft) -> Result<(), Vec<String>> {
        let mut reasons = Vec::new();
        let count = ladder.concepts.len();
        if !(MIN_CONCEPTS..=MAX_CONCEPTS).contains(&count) {
            reasons.push(format!(
                "the ladder has {count} concepts; it needs {MIN_CONCEPTS} to {MAX_CONCEPTS}"
            ));
        }
        let mut seen = HashSet::new();
        for (index, concept) in ladder.concepts.iter().enumerate() {
            let name = concept.trim().to_lowercase();
            if name.is_empty() {
                reasons.push(format!("concept {} is empty", index + 1));
            } else if !seen.insert(name) {
                reasons.push(format!(
                    "concept {} repeats an earlier concept, {concept:?}",
                    index + 1
                ));
            }
        }
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(reasons)
        }
    }
}

/// The draft ladder task's input.
pub(super) fn ladder_input(topic: &str) -> String {
    format!("Topic: {topic}")
}

/// A placement question as the model writes it.
#[derive(Debug, Deserialize)]
pub(super) struct QuestionDraft {
    stem: String,
    options: Vec<OptionDraft>,
    key: u32,
}

#[derive(Debug, Deserialize)]
struct OptionDraft {
    text: String,
    misconception: Option<String>,
}

impl QuestionDraft {
    /// The question tied to `rung`, with its options turned by `turn`
    /// places so the key's position does not follow the model's habits.
    pub(super) fn into_question(self, rung: RungId, turn: usize) -> KeyedPlacementQuestion {
        let mut options: Vec<KeyedOption> = self
            .options
            .into_iter()
            .map(|option| KeyedOption {
                text: option.text.trim().to_owned(),
                misconception: option
                    .misconception
                    .map(|label| label.trim().to_owned())
                    .filter(|label| !label.is_empty()),
            })
            .collect();
        let turn = turn % options.len();
        options.rotate_right(turn);
        KeyedPlacementQuestion {
            id: Uuid::new_v4(),
            rung,
            stem: self.stem.trim().to_owned(),
            key: ((self.key as usize + turn) % OPTIONS) as u32,
            options,
        }
    }
}

pub(super) struct PlacementQuestion;

impl StructuredTask for PlacementQuestion {
    type Output = QuestionDraft;

    fn task(&self) -> Task {
        Task::PlacementQuestion
    }

    fn prompt(&self) -> Prompt {
        prompts::PLACEMENT_QUESTION
    }

    fn schema(&self) -> OutputSchema {
        OutputSchema {
            name: "placement_question".to_owned(),
            schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["stem", "options", "key"],
                "properties": {
                    "stem": {
                        "type": "string",
                        "description": "The question: what to do or what happens next."
                    },
                    "options": {
                        "type": "array",
                        "description": "Exactly four options, one right.",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["text", "misconception"],
                            "properties": {
                                "text": { "type": "string" },
                                "misconception": {
                                    "type": ["string", "null"],
                                    "description": "For a wrong option, the specific \
                                                    misconception it represents; null for \
                                                    the right one."
                                }
                            }
                        }
                    },
                    "key": {
                        "type": "integer",
                        "description": "The position of the right option, counted from 0."
                    }
                }
            }),
        }
    }

    fn check(&self, question: &QuestionDraft) -> Result<(), Vec<String>> {
        let mut reasons = Vec::new();
        if question.stem.trim().is_empty() {
            reasons.push("the stem is empty".to_owned());
        }
        let count = question.options.len();
        if count != OPTIONS {
            reasons.push(format!(
                "the question has {count} options; it needs {OPTIONS}"
            ));
        }
        let key = question.key as usize;
        if key >= count {
            reasons.push(format!("the key {key} names no option"));
        }
        let mut texts = HashSet::new();
        for (index, option) in question.options.iter().enumerate() {
            let text = option.text.trim().to_lowercase();
            if text.is_empty() {
                reasons.push(format!("option {index} is empty"));
            } else if !texts.insert(text) {
                reasons.push(format!("option {index} repeats an earlier option"));
            }
            let labelled = option
                .misconception
                .as_deref()
                .is_some_and(|label| !label.trim().is_empty());
            if index == key && labelled {
                reasons.push(format!(
                    "option {index} is the key, so its misconception must be null"
                ));
            } else if index != key && !labelled {
                reasons.push(format!(
                    "option {index} is wrong, so it needs the misconception it represents"
                ));
            }
        }
        if reasons.is_empty() {
            Ok(())
        } else {
            Err(reasons)
        }
    }
}

/// The placement question task's input: the topic, the goal, the rung's
/// concept and the questions already asked. No other rung is named.
pub(super) fn question_input(
    topic: &str,
    goal: Goal,
    rung: &Rung,
    ladder_len: usize,
    asked: &[&str],
) -> String {
    let mut input = format!(
        "Topic: {topic}\nGoal: {}\nConcept to test: {}\nIts place on the ladder: rung {} of \
         {ladder_len}, counted from the most basic\n",
        goal_text(goal),
        rung.concept,
        rung.id.0,
    );
    if !asked.is_empty() {
        input.push_str("\nQuestions already asked:\n");
        for stem in asked {
            let _ = writeln!(input, "- {stem}");
        }
    }
    input
}

/// The goal in the words the prompts use.
pub(super) fn goal_text(goal: Goal) -> &'static str {
    match goal {
        Goal::Overview => "overview: the shape of the whole topic",
        Goal::WorkingKnowledge => "working knowledge: able to use it in practice",
        Goal::Expert => "expert: depth, trade-offs and the edge cases",
    }
}
