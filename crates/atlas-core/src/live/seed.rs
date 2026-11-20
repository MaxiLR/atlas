//! What the throwaway course needs from a task that is not selected, written
//! straight to the store with no request: a ladder, the placement answers
//! that finish the search, an index with its map, and a written section.

use uuid::Uuid;

use crate::domain::course_map::{Concept, CourseMap, MapEntry};
use crate::domain::ladder::{Ladder, Rung, RungId};
use crate::domain::questions::{KeyedOption, KeyedPlacementQuestion};
use crate::domain::record::{RecordStatus, SectionRecord, StoredRecord, TaughtConcept};
use crate::index_tree::{IndexTree, TreeChapter, TreeSection};
use crate::placement::{MAX_QUESTIONS, SearchAnswer, next_rung, profile};
use crate::protocol::{
    Block, BlockContent, CoreError, Goal, PlacementAnswer, SelfRating, VersionStyle,
};
use crate::store::{NewSectionVersion, Store};

/// The course every run starts.
pub(super) const TOPIC: &str = "Plate tectonics";
pub(super) const RATING: SelfRating = SelfRating::KnowTheBasics;
pub(super) const GOAL: Goal = Goal::WorkingKnowledge;

/// Eight concepts, the most basic first.
pub(super) fn ladder() -> Ladder {
    let concepts = [
        "The Earth's layers",
        "The lithosphere",
        "Plates",
        "Mantle convection",
        "Plate boundaries",
        "Ridges",
        "Subduction",
        "Mountain building",
    ];
    Ladder {
        rungs: (1..)
            .zip(concepts)
            .map(|(id, concept)| Rung {
                id: RungId(id),
                concept: concept.to_owned(),
            })
            .collect(),
    }
}

/// Answers the question waiting, "Not sure", then the search's next
/// questions, each a seeded question answered right, until the search is
/// finished. Answers the profile the answers give.
pub(super) fn finish_placement(store: &Store, course: Uuid) -> Result<(), CoreError> {
    let ladder = store.ladder(course)?.unwrap_or_else(ladder);
    let mut answers = Vec::new();
    for asked in store.placement_questions(course)? {
        let answer = match asked.answer {
            Some(PlacementAnswer::Option { index }) if index == asked.question.key => {
                SearchAnswer::Right
            }
            Some(PlacementAnswer::Option { index }) => SearchAnswer::Wrong {
                misconception: asked
                    .question
                    .options
                    .get(index as usize)
                    .and_then(|option| option.misconception.clone()),
            },
            Some(PlacementAnswer::NotSure) => SearchAnswer::NotSure,
            None => {
                store.answer_placement_question(asked.question.id, PlacementAnswer::NotSure)?;
                SearchAnswer::NotSure
            }
        };
        answers.push(answer);
    }
    for _ in 0..=MAX_QUESTIONS {
        let Some(rung) = next_rung(ladder.rungs.len(), RATING, &answers) else {
            break;
        };
        let rung = &ladder.rungs[rung];
        let option = |text: String, misconception: Option<&str>| KeyedOption {
            text,
            misconception: misconception.map(str::to_owned),
        };
        let question = KeyedPlacementQuestion {
            id: Uuid::new_v4(),
            rung: rung.id,
            stem: format!("A seeded question on {}.", rung.concept),
            options: vec![
                option("The right answer".to_owned(), None),
                option("A wrong answer".to_owned(), Some("seeded")),
                option("Another wrong answer".to_owned(), Some("seeded")),
                option("A third wrong answer".to_owned(), Some("seeded")),
            ],
            key: 0,
        };
        store.add_placement_question(course, &question)?;
        store.answer_placement_question(question.id, PlacementAnswer::Option { index: 0 })?;
        answers.push(SearchAnswer::Right);
    }
    store.save_profile(course, &profile(&ladder, RATING, GOAL, &answers))?;
    Ok(())
}

/// An index of two chapters and three sections with its map, no conventions,
/// moving the course to the ready stage.
pub(super) fn index(store: &Store, course: Uuid) -> Result<(), CoreError> {
    let section = |title: &str, line: &str| TreeSection {
        id: Uuid::new_v4(),
        title: title.to_owned(),
        line: line.to_owned(),
        read: false,
        children: Vec::new(),
    };
    let layers = section("The Earth's layers", "Crust, mantle and core.");
    let plates = section("Plates", "The rigid pieces of the lithosphere.");
    let motion = section("Why plates move", "The mantle beneath them.");
    let entry = |section: &TreeSection, introduces: &[&str], relies_on: &[&str]| MapEntry {
        section: section.id,
        introduces: introduces.iter().map(|name| (*name).to_owned()).collect(),
        relies_on: relies_on.iter().map(|name| (*name).to_owned()).collect(),
    };
    let map = CourseMap {
        concepts: ["the Earth's layers", "plates", "mantle convection"]
            .into_iter()
            .map(|name| Concept {
                name: name.to_owned(),
                rung: None,
            })
            .collect(),
        entries: vec![
            entry(&layers, &["the Earth's layers"], &[]),
            entry(&plates, &["plates"], &["the Earth's layers"]),
            entry(&motion, &["mantle convection"], &["plates"]),
        ],
    };
    let tree = IndexTree {
        chapters: vec![
            TreeChapter {
                id: Uuid::new_v4(),
                title: "What the Earth is made of".to_owned(),
                sections: vec![layers, plates],
            },
            TreeChapter {
                id: Uuid::new_v4(),
                title: "How plates move".to_owned(),
                sections: vec![motion],
            },
        ],
    };
    store.insert_index(course, &tree, &map, &[])?;
    Ok(())
}

/// A written version of `section`, with a confirmed record, made current.
pub(super) fn version(store: &Store, section: Uuid) -> Result<(), CoreError> {
    let text = "The Earth is built in layers. A thin, cold crust sits on a hot, slowly \
                moving mantle, which wraps a metal core.";
    let version = NewSectionVersion {
        id: Uuid::new_v4(),
        node: section,
        style: VersionStyle::Original,
        blocks: vec![Block {
            index: 0,
            content: BlockContent::Prose {
                text: text.to_owned(),
            },
        }],
        checks: Vec::new(),
        citations: Vec::new(),
        sources: Vec::new(),
        written_without_search: false,
        record: StoredRecord {
            record: SectionRecord {
                summary: Some(
                    "The Earth has a crust, a mantle and a core. The mantle moves slowly."
                        .to_owned(),
                ),
                concepts: vec![TaughtConcept {
                    name: "the Earth's layers".to_owned(),
                    line: Some("A thin crust on a moving mantle around a core.".to_owned()),
                }],
                notation: Vec::new(),
                terms: Vec::new(),
                examples: Vec::new(),
                promises: Vec::new(),
            },
            status: RecordStatus::Confirmed,
        },
        model: None,
        prompt: "live-command-seed".to_owned(),
    };
    store.insert_version(&version, &[])?;
    Ok(())
}
