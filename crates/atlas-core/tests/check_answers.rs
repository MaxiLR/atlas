//! Check questions through the core's request handler: an answer to a
//! question of a stored version says whether it was right, with the key and
//! the explanation; the first answer moves the question's rung one step on
//! the candidate profile and records the misconception of a wrong option; a
//! repeated answer changes nothing; no model request is made; and the next
//! section's request carries the rung's new state.
//!
//! The sample course is laid down through the store's repository API on the
//! same database file in a temp dir, with one more version of "Layers of the
//! Earth" that ends in check questions. Expected profiles are written out by
//! hand from the sample profile: rungs 1 and 2 known, 3 shaky, 4 to 8 unknown.

mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionOrigin};
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::profile::{CandidateProfile, Knowledge, Misconception, RungKnowledge};
use atlas_core::domain::questions::{KeyedCheckQuestion, KeyedOption};
use atlas_core::domain::record::StoredRecord;
use atlas_core::protocol::{
    Block, BlockContent, CheckQuestion, CheckResult, CoreError, CoreRequest, CoreResponse, Goal,
    VersionStyle,
};
use atlas_core::provider::Task;
use atlas_core::store::{CheckBlock, NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, FakeProvider, FakeScript, RecordingEvents};
use support::sample_course::{LAYERS, SampleCourse, WHAT_DRIVES_PLATES, sample_course};
use tempfile::TempDir;
use uuid::Uuid;

/// The version of "Layers of the Earth" that carries the check questions.
const CHECKED: Uuid = Uuid::from_u128(0xc4ec);
/// Its check block, after the prose.
const CHECK_BLOCK: u32 = 1;
/// A second check block, after more prose.
const SECOND_CHECK_BLOCK: u32 = 3;

const BOUNDARY_MISCONCEPTION: &str = "Thinks plates only ever collide";
const CRUST_MISCONCEPTION: &str = "Thinks the crust is the thickest layer";
const CRUST_EXPLANATION: &str =
    "The crust is a thin skin: a few to a few tens of kilometres against thousands for the mantle.";
const BOUNDARY_EXPLANATION: &str =
    "Plates can pull apart, collide or slide past each other; each makes a different boundary.";
const RIDGE_EXPLANATION: &str =
    "At a mid-ocean ridge two plates pull apart and new crust forms between them.";
const CONVECTION_EXPLANATION: &str =
    "Heat escaping from the interior keeps the mantle convecting, and the plates ride on it.";

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sections")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn option(text: &str, misconception: Option<&str>) -> KeyedOption {
    KeyedOption {
        text: text.to_owned(),
        misconception: misconception.map(str::to_owned),
    }
}

/// The first check block: a question on rung 5 (plate boundaries, unknown)
/// keyed 2, and one on rung 1 (Earth's layers, known) keyed 0.
fn first_block() -> Vec<KeyedCheckQuestion> {
    vec![
        KeyedCheckQuestion {
            rung: RungId(5),
            stem: "Which of these is a kind of plate boundary?".to_owned(),
            options: vec![
                option("Only collisions", Some(BOUNDARY_MISCONCEPTION)),
                option(
                    "Only rifts on land",
                    Some("Thinks boundaries are only rifts"),
                ),
                option("Pulling apart, colliding or sliding past", None),
                option(
                    "None: plates have no edges",
                    Some("Thinks plates are endless"),
                ),
            ],
            key: 2,
            explanation: BOUNDARY_EXPLANATION.to_owned(),
        },
        KeyedCheckQuestion {
            rung: RungId(1),
            stem: "Which layer is the thinnest?".to_owned(),
            options: vec![
                option("The crust", None),
                option("The mantle", Some(CRUST_MISCONCEPTION)),
                option("The outer core", Some("Thinks the core is a thin shell")),
                option("They are all as thick", Some("Thinks the layers are equal")),
            ],
            key: 0,
            explanation: CRUST_EXPLANATION.to_owned(),
        },
    ]
}

/// The second check block: another question on rung 5 keyed 1, and one on
/// rung 4 (mantle convection, unknown) keyed 3.
fn second_block() -> Vec<KeyedCheckQuestion> {
    vec![
        KeyedCheckQuestion {
            rung: RungId(5),
            stem: "What happens at a mid-ocean ridge?".to_owned(),
            options: vec![
                option("Two plates collide", Some(BOUNDARY_MISCONCEPTION)),
                option("Two plates pull apart", None),
                option("One plate sinks", Some("Thinks ridges are trenches")),
                option("Nothing moves", Some("Thinks ridges are still")),
            ],
            key: 1,
            explanation: RIDGE_EXPLANATION.to_owned(),
        },
        KeyedCheckQuestion {
            rung: RungId(4),
            stem: "What keeps the mantle moving?".to_owned(),
            options: vec![
                option("The Moon's pull", Some("Thinks tides move the mantle")),
                option("The winds", Some("Thinks the atmosphere moves the mantle")),
                option("Nothing: it is still", Some("Thinks the mantle is still")),
                option("Heat escaping from the interior", None),
            ],
            key: 3,
            explanation: CONVECTION_EXPLANATION.to_owned(),
        },
    ]
}

fn prose(index: u32, text: &str) -> Block {
    Block {
        index,
        content: BlockContent::Prose {
            text: text.to_owned(),
        },
    }
}

fn shown(index: u32, questions: &[KeyedCheckQuestion]) -> Block {
    Block {
        index,
        content: BlockContent::Check {
            questions: (0..)
                .zip(questions)
                .map(|(number, question)| CheckQuestion {
                    index: number,
                    stem: question.stem.clone(),
                    options: question
                        .options
                        .iter()
                        .map(|option| option.text.clone())
                        .collect(),
                    answer: None,
                })
                .collect(),
        },
    }
}

/// A version of "Layers of the Earth": prose, a check block, prose, another
/// check block.
fn checked_version(record: StoredRecord) -> NewSectionVersion {
    NewSectionVersion {
        id: CHECKED,
        node: LAYERS,
        style: VersionStyle::Original,
        blocks: vec![
            prose(0, "The Earth is built in layers."),
            shown(CHECK_BLOCK, &first_block()),
            prose(2, "The plates meet at their edges."),
            shown(SECOND_CHECK_BLOCK, &second_block()),
        ],
        checks: vec![
            CheckBlock {
                block: CHECK_BLOCK,
                questions: first_block(),
            },
            CheckBlock {
                block: SECOND_CHECK_BLOCK,
                questions: second_block(),
            },
        ],
        citations: Vec::new(),
        sources: Vec::new(),
        written_without_search: false,
        record,
        model: Some("gpt-5".to_owned()),
        prompt: "write-section@1".to_owned(),
    }
}

fn prose_version(id: Uuid, node: Uuid, record: StoredRecord) -> NewSectionVersion {
    NewSectionVersion {
        id,
        node,
        style: VersionStyle::Original,
        blocks: vec![prose(0, "The text of an earlier section.")],
        checks: Vec::new(),
        citations: Vec::new(),
        sources: Vec::new(),
        written_without_search: false,
        record,
        model: Some("gpt-5".to_owned()),
        prompt: "write-section@1".to_owned(),
    }
}

/// Lays the course down: ladder, profile, index with its map and the plan's
/// conventions, a version for every record, then the checked version of
/// "Layers of the Earth" as its current one.
fn store_course(store: &Store, sample: &SampleCourse) -> Uuid {
    let course = store.create_course(&sample.topic).unwrap().id;
    store.store_ladder(course, &sample.ladder).unwrap();
    store.save_profile(course, &sample.profile).unwrap();
    let fixed_by = |origin: ConventionOrigin| -> Vec<Convention> {
        sample
            .conventions
            .iter()
            .filter(|convention| convention.origin == origin)
            .cloned()
            .collect()
    };
    store
        .insert_index(
            course,
            &sample.index,
            &sample.map,
            &fixed_by(ConventionOrigin::Plan),
        )
        .unwrap();
    for record in &sample.records {
        store
            .insert_version(
                &prose_version(record.version, record.section, record.record.clone()),
                &fixed_by(ConventionOrigin::SectionVersion {
                    version: record.version,
                }),
            )
            .unwrap();
    }
    let layers = &sample.records[0];
    assert_eq!(layers.section, LAYERS);
    store
        .insert_version(&checked_version(layers.record.clone()), &[])
        .unwrap();
    course
}

struct Harness {
    _dir: TempDir,
    provider: Arc<FakeProvider>,
    core: Core,
    store: Store,
    course: Uuid,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(
            FakeProvider::new(FakeScript::sample())
                .answer(Task::WriteSection, [Ok(fixture("complete.md"))]),
        );
        let events = Arc::new(RecordingEvents::default());
        let core =
            Core::new(ports.deps(path.clone(), provider.clone(), events)).expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        let course = store_course(&store, &sample_course());
        Self {
            _dir: dir,
            provider,
            core,
            store,
            course,
        }
    }

    async fn answer(
        &self,
        version_id: Uuid,
        block: u32,
        question: u32,
        choice: u32,
    ) -> Result<CheckResult, CoreError> {
        self.core
            .handle(CoreRequest::AnswerCheck {
                version_id,
                block,
                question,
                choice,
            })
            .await
            .map(|response| match response {
                CoreResponse::CheckAnswered { result } => result,
                other => panic!("expected a check result, got {other:?}"),
            })
    }

    fn profile(&self) -> CandidateProfile {
        self.store
            .profile(self.course)
            .unwrap()
            .expect("the course has a profile")
    }

    fn state(&self, rung: u32) -> Knowledge {
        self.profile()
            .rungs
            .iter()
            .find(|known| known.rung == RungId(rung))
            .unwrap_or_else(|| panic!("rung {rung} is on the profile"))
            .state
    }
}

/// The sample profile with the given rungs changed and misconceptions added.
fn expected_profile(
    changed: &[(u32, Knowledge)],
    misconceptions: &[(u32, &str)],
) -> CandidateProfile {
    let state = |rung: u32| {
        changed
            .iter()
            .find(|(changed, _)| *changed == rung)
            .map(|(_, state)| *state)
            .unwrap_or(match rung {
                1 | 2 => Knowledge::Known,
                3 => Knowledge::Shaky,
                _ => Knowledge::Unknown,
            })
    };
    let mut recorded = vec![Misconception {
        rung: RungId(4),
        text: "The mantle is a sea of liquid rock.".to_owned(),
    }];
    recorded.extend(misconceptions.iter().map(|(rung, text)| Misconception {
        rung: RungId(*rung),
        text: (*text).to_owned(),
    }));
    CandidateProfile {
        goal: Goal::WorkingKnowledge,
        rungs: (1..=8)
            .map(|rung| RungKnowledge {
                rung: RungId(rung),
                state: state(rung),
            })
            .collect(),
        misconceptions: recorded,
    }
}

#[tokio::test]
async fn a_right_answer_on_an_unknown_rung_makes_it_shaky_and_another_makes_it_known() {
    let harness = Harness::new();
    assert_eq!(harness.state(5), Knowledge::Unknown);

    let result = harness.answer(CHECKED, CHECK_BLOCK, 0, 2).await;

    assert_eq!(
        result,
        Ok(CheckResult {
            choice: 2,
            correct: true,
            key: 2,
            explanation: BOUNDARY_EXPLANATION.to_owned(),
        })
    );
    assert_eq!(
        harness.profile(),
        expected_profile(&[(5, Knowledge::Shaky)], &[])
    );

    let result = harness.answer(CHECKED, SECOND_CHECK_BLOCK, 0, 1).await;

    assert_eq!(
        result,
        Ok(CheckResult {
            choice: 1,
            correct: true,
            key: 1,
            explanation: RIDGE_EXPLANATION.to_owned(),
        })
    );
    assert_eq!(
        harness.profile(),
        expected_profile(&[(5, Knowledge::Known)], &[])
    );
}

#[tokio::test]
async fn a_wrong_answer_on_a_known_rung_makes_it_shaky_and_records_the_misconception() {
    let harness = Harness::new();
    assert_eq!(harness.state(1), Knowledge::Known);

    let result = harness.answer(CHECKED, CHECK_BLOCK, 1, 1).await;

    assert_eq!(
        result,
        Ok(CheckResult {
            choice: 1,
            correct: false,
            key: 0,
            explanation: CRUST_EXPLANATION.to_owned(),
        })
    );
    assert_eq!(
        harness.profile(),
        expected_profile(&[(1, Knowledge::Shaky)], &[(1, CRUST_MISCONCEPTION)])
    );
}

#[tokio::test]
async fn a_wrong_answer_on_an_unknown_rung_leaves_it_unknown_and_records_the_misconception() {
    let harness = Harness::new();

    let result = harness.answer(CHECKED, CHECK_BLOCK, 0, 0).await;

    assert_eq!(
        result,
        Ok(CheckResult {
            choice: 0,
            correct: false,
            key: 2,
            explanation: BOUNDARY_EXPLANATION.to_owned(),
        })
    );
    assert_eq!(
        harness.profile(),
        expected_profile(&[], &[(5, BOUNDARY_MISCONCEPTION)])
    );
}

#[tokio::test]
async fn the_first_answer_is_stored() {
    let harness = Harness::new();

    harness
        .answer(CHECKED, CHECK_BLOCK, 1, 1)
        .await
        .expect("answered");

    let stored = harness
        .store
        .check_answer(CHECKED, CHECK_BLOCK, 1)
        .unwrap()
        .expect("the answer is stored");
    assert_eq!(
        (stored.version_id, stored.block, stored.question),
        (CHECKED, CHECK_BLOCK, 1)
    );
    assert_eq!((stored.choice, stored.correct), (1, false));
    assert_eq!(harness.store.check_answers(CHECKED).unwrap().len(), 1);
}

#[tokio::test]
async fn a_repeated_answer_returns_the_stored_result_and_changes_nothing() {
    let harness = Harness::new();
    let first = harness
        .answer(CHECKED, CHECK_BLOCK, 1, 1)
        .await
        .expect("answered");
    let profile = harness.profile();
    let answers = harness.store.check_answers(CHECKED).unwrap();

    // The same choice again, then the right one.
    let same = harness.answer(CHECKED, CHECK_BLOCK, 1, 1).await;
    let other = harness.answer(CHECKED, CHECK_BLOCK, 1, 0).await;

    assert_eq!(same, Ok(first.clone()));
    assert_eq!(other, Ok(first));
    assert_eq!(
        other.unwrap(),
        CheckResult {
            choice: 1,
            correct: false,
            key: 0,
            explanation: CRUST_EXPLANATION.to_owned(),
        }
    );
    assert_eq!(harness.profile(), profile);
    assert_eq!(harness.store.check_answers(CHECKED).unwrap(), answers);
}

#[tokio::test]
async fn answering_makes_no_model_request() {
    let harness = Harness::new();

    harness
        .answer(CHECKED, CHECK_BLOCK, 0, 2)
        .await
        .expect("answered");
    harness
        .answer(CHECKED, CHECK_BLOCK, 1, 3)
        .await
        .expect("answered");
    harness
        .answer(CHECKED, CHECK_BLOCK, 1, 3)
        .await
        .expect("answered");

    assert_eq!(harness.provider.requests(), vec![]);
}

#[tokio::test]
async fn the_next_section_request_carries_the_rungs_new_state() {
    let harness = Harness::new();
    // Rung 4 unknown to shaky; rung 1 known to shaky, with a misconception.
    harness
        .answer(CHECKED, SECOND_CHECK_BLOCK, 1, 3)
        .await
        .expect("answered");
    harness
        .answer(CHECKED, CHECK_BLOCK, 1, 1)
        .await
        .expect("answered");

    harness
        .core
        .handle(CoreRequest::OpenSection {
            course_id: harness.course,
            section_id: WHAT_DRIVES_PLATES,
        })
        .await
        .expect("the section is written");

    let requests: Vec<_> = harness
        .provider
        .requests()
        .into_iter()
        .filter(|request| request.task == Task::WriteSection)
        .collect();
    assert_eq!(requests.len(), 1);
    let input = &requests[0].input;
    assert!(
        input.contains("- rung 4, Mantle convection: shaky"),
        "{input}"
    );
    assert!(input.contains("- rung 1, Earth's layers: shaky"), "{input}");
    assert!(
        input.contains(&format!("- rung 1: {CRUST_MISCONCEPTION}")),
        "{input}"
    );
    assert!(
        !input.contains("- rung 4, Mantle convection: unknown"),
        "{input}"
    );
}

#[tokio::test]
async fn an_answer_to_a_missing_version_block_question_or_option_is_an_error() {
    let harness = Harness::new();
    let profile = harness.profile();

    let missing_version = harness
        .answer(Uuid::from_u128(0xdead), CHECK_BLOCK, 0, 0)
        .await;
    let missing_block = harness.answer(CHECKED, 9, 0, 0).await;
    let prose_block = harness.answer(CHECKED, 0, 0, 0).await;
    let missing_question = harness.answer(CHECKED, CHECK_BLOCK, 2, 0).await;
    let missing_option = harness.answer(CHECKED, CHECK_BLOCK, 0, 4).await;

    for (what, result) in [
        ("a missing version", missing_version),
        ("a missing block", missing_block),
        ("a block without questions", prose_block),
        ("a missing question", missing_question),
        ("a missing option", missing_option),
    ] {
        match result {
            Err(CoreError::NotFound { .. } | CoreError::InvalidRequest { .. }) => {}
            other => panic!("{what}: expected not found or invalid, got {other:?}"),
        }
    }
    assert_eq!(harness.profile(), profile);
    assert_eq!(harness.store.check_answers(CHECKED).unwrap(), vec![]);
    assert_eq!(harness.provider.requests(), vec![]);
}
