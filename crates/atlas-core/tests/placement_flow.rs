//! Placement through the core's request handler: a course created in the
//! placement stage, the self-rating and goal, the hidden ladder, one question
//! at a time on the rung the search picks, the profile and the index build,
//! with a scripted provider and a database in a temp dir.
//!
//! Rungs are counted from 1 at the most basic, as the ladder's ids are. The
//! rungs asked and the profiles expected are worked out by hand from the
//! spec's placement rules.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use atlas_core::Core;
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::profile::{CandidateProfile, Knowledge, Misconception, RungKnowledge};
use atlas_core::protocol::{
    CoreError, CoreEvent, CoreRequest, CoreResponse, CourseStage, CourseView, Goal,
    PlacementAnswer, PlacementQuestion, PlacementState, PlacementStep, SelfRating,
};
use atlas_core::provider::{FakeProvider, FakeScript, ProviderError, ProviderRequest, Task};
use atlas_core::store::Store;
use atlas_core::testing::{FakeClock, FakePorts, RecordingEvents};
use serde_json::json;
use uuid::Uuid;

const TOPIC: &str = "Plate tectonics";

// -- the script ----------------------------------------------------------------

/// The concept of rung `rung` (from 1): a sentinel no event or response may
/// carry.
fn concept(rung: usize) -> String {
    format!("ladder concept {rung:02} zq")
}

/// A ladder answer of `len` concepts in prerequisite order.
fn ladder(len: usize) -> String {
    let concepts: Vec<String> = (1..=len).map(concept).collect();
    json!({ "concepts": concepts }).to_string()
}

fn stem(n: usize) -> String {
    format!("Question {n}: a plate slides under another; what happens next?")
}

fn right(n: usize) -> String {
    format!("Right option {n}")
}

/// Wrong option `k` (1 to 3) of question `n`.
fn wrong(n: usize, k: usize) -> String {
    format!("Wrong option {n}.{k}")
}

/// The misconception wrong option `k` of question `n` represents: a sentinel
/// no event or response may carry.
fn label(n: usize, k: usize) -> String {
    format!("misconception label {n}.{k} zq")
}

/// Question `n` as the model writes it: four options, the right one at a
/// position that moves with `n`, each wrong one labelled.
fn question(n: usize) -> String {
    let key = n % 4;
    let mut wrongs = 1..=3;
    let options: Vec<serde_json::Value> = (0..4)
        .map(|position| {
            if position == key {
                json!({ "text": right(n), "misconception": null })
            } else {
                let k = wrongs.next().unwrap();
                json!({ "text": wrong(n, k), "misconception": label(n, k) })
            }
        })
        .collect();
    json!({ "stem": stem(n), "options": options, "key": key }).to_string()
}

fn ok(text: String) -> Result<String, ProviderError> {
    Ok(text)
}

/// The sentinels of a script: every concept of a ladder up to 13 rungs and
/// every misconception label of six questions.
fn secrets() -> Vec<String> {
    let mut secrets: Vec<String> = (1..=13).map(concept).collect();
    for n in 1..=6 {
        for k in 1..=3 {
            secrets.push(label(n, k));
        }
    }
    secrets
}

/// What the candidate picks.
#[derive(Debug, Clone, Copy)]
enum Pick {
    Right,
    /// Wrong option `k`.
    Wrong(usize),
    NotSure,
}

// -- the harness ---------------------------------------------------------------

struct Harness {
    core: Core,
    provider: Arc<FakeProvider>,
    events: Arc<RecordingEvents>,
    /// Every response, error and event so far, serialised.
    seen: Mutex<Vec<String>>,
}

fn start(database: &Path, provider: FakeProvider) -> Harness {
    let provider = Arc::new(provider);
    let events = Arc::new(RecordingEvents::default());
    let core =
        Core::new(FakePorts::default().deps(database.to_owned(), provider.clone(), events.clone()))
            .expect("core starts");
    Harness {
        core,
        provider,
        events,
        seen: Mutex::new(Vec::new()),
    }
}

fn database(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("atlas.sqlite3")
}

impl Harness {
    async fn send(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        let result = self.core.handle(request).await;
        let text = match &result {
            Ok(response) => serde_json::to_string(response).unwrap(),
            Err(error) => serde_json::to_string(error).unwrap(),
        };
        self.seen.lock().unwrap().push(text);
        result
    }

    async fn course_response(&self, request: CoreRequest) -> CourseView {
        match self.send(request).await {
            Ok(CoreResponse::Course { course }) => course,
            other => panic!("expected a course, got {other:?}"),
        }
    }

    async fn create_course(&self, topic: &str) -> CourseView {
        self.course_response(CoreRequest::CreateCourse {
            topic: topic.to_owned(),
        })
        .await
    }

    async fn open_course(&self, course_id: Uuid) -> CourseView {
        self.course_response(CoreRequest::OpenCourse { course_id })
            .await
    }

    async fn placement(&self, course: Uuid, request: CoreRequest) -> PlacementState {
        match self.send(request).await {
            Ok(CoreResponse::Placement {
                course_id,
                placement,
            }) => {
                assert_eq!(course_id, course);
                placement
            }
            other => panic!("expected a placement state, got {other:?}"),
        }
    }

    async fn submit(&self, course_id: Uuid, rating: SelfRating, goal: Goal) -> PlacementState {
        self.placement(
            course_id,
            CoreRequest::SubmitSelfRating {
                course_id,
                rating,
                goal,
            },
        )
        .await
    }

    async fn answer(
        &self,
        course_id: Uuid,
        shown: &PlacementQuestion,
        pick: Pick,
    ) -> PlacementState {
        self.placement(
            course_id,
            CoreRequest::AnswerPlacement {
                course_id,
                question_id: shown.id,
                answer: answer(shown, pick),
            },
        )
        .await
    }

    async fn retry(&self, course_id: Uuid) -> PlacementState {
        self.placement(course_id, CoreRequest::RetryPlacementStep { course_id })
            .await
    }

    /// The events emitted since the last call.
    fn events(&self) -> Vec<CoreEvent> {
        let events = self.events.take();
        let mut seen = self.seen.lock().unwrap();
        seen.extend(
            events
                .iter()
                .map(|event| serde_json::to_string(event).unwrap()),
        );
        events
    }

    /// The placement states reported for `course` since the last call.
    fn progress(&self, course: Uuid) -> Vec<PlacementState> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                CoreEvent::PlacementProgress {
                    course_id,
                    placement,
                } if course_id == course => Some(placement),
                _ => None,
            })
            .collect()
    }

    fn tasks(&self) -> Vec<Task> {
        self.provider.requests().iter().map(|r| r.task).collect()
    }

    fn requests(&self, task: Task) -> Vec<ProviderRequest> {
        self.provider
            .requests()
            .into_iter()
            .filter(|request| request.task == task)
            .collect()
    }

    /// Fails if any response, error or event carries a ladder concept, a
    /// misconception label or an answer key.
    fn assert_nothing_leaked(&self) {
        self.events();
        let secrets = secrets();
        for text in self.seen.lock().unwrap().iter() {
            for secret in &secrets {
                assert!(
                    !text.contains(secret.as_str()),
                    "{secret:?} leaked in {text}"
                );
            }
            assert!(!text.contains("\"key\""), "a key leaked in {text}");
            assert!(!text.contains("isconception"), "a label leaked in {text}");
        }
    }
}

/// The answer that picks `pick` on the question shown, found by the option's
/// text: the core may place the options in any order.
fn answer(shown: &PlacementQuestion, pick: Pick) -> PlacementAnswer {
    let n = shown.number as usize;
    let text = match pick {
        Pick::Right => right(n),
        Pick::Wrong(k) => wrong(n, k),
        Pick::NotSure => return PlacementAnswer::NotSure,
    };
    let index = shown
        .options
        .iter()
        .position(|option| *option == text)
        .unwrap_or_else(|| panic!("{text:?} is not among {:?}", shown.options));
    PlacementAnswer::Option {
        index: index as u32,
    }
}

fn shown(state: &PlacementState) -> PlacementQuestion {
    match &state.step {
        PlacementStep::Question { question } => question.clone(),
        other => panic!("expected a question, got {other:?} in {state:?}"),
    }
}

fn state(step: PlacementStep) -> PlacementState {
    PlacementState {
        step,
        min_questions: 4,
        max_questions: 6,
        error: None,
    }
}

/// The steps reported, with whether each carried an error.
fn steps(states: &[PlacementState]) -> Vec<(String, bool)> {
    states
        .iter()
        .map(|state| {
            let step = match &state.step {
                PlacementStep::ChooseRating => "ChooseRating".to_owned(),
                PlacementStep::PreparingQuestions => "PreparingQuestions".to_owned(),
                PlacementStep::Question { question } => format!("Question {}", question.number),
                PlacementStep::BuildingIndex => "BuildingIndex".to_owned(),
                PlacementStep::Done => "Done".to_owned(),
            };
            assert_eq!((state.min_questions, state.max_questions), (4, 6));
            (step, state.error.is_some())
        })
        .collect()
}

fn expect_steps(expected: &[(&str, bool)]) -> Vec<(String, bool)> {
    expected
        .iter()
        .map(|(step, error)| ((*step).to_owned(), *error))
        .collect()
}

/// The one ladder concept a question request names, as its rung.
fn asked_rung(request: &ProviderRequest, ladder_len: usize) -> usize {
    let named: Vec<usize> = (1..=ladder_len)
        .filter(|rung| request.input.contains(&concept(*rung)))
        .collect();
    assert_eq!(named.len(), 1, "{}", request.input);
    named[0]
}

fn profile(goal: Goal, states: &[Knowledge], misconceptions: &[(u32, String)]) -> CandidateProfile {
    CandidateProfile {
        goal,
        rungs: (1..)
            .zip(states)
            .map(|(rung, state)| RungKnowledge {
                rung: RungId(rung),
                state: *state,
            })
            .collect(),
        misconceptions: misconceptions
            .iter()
            .map(|(rung, text)| Misconception {
                rung: RungId(*rung),
                text: text.clone(),
            })
            .collect(),
    }
}

fn stored_profile(database: &Path, course: Uuid) -> Option<CandidateProfile> {
    Store::open(database, Arc::new(FakeClock::default()))
        .unwrap()
        .profile(course)
        .unwrap()
}

// -- creating a course ---------------------------------------------------------

#[tokio::test]
async fn creating_a_course_stores_it_in_the_placement_stage_without_a_request() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&database(&dir), FakeProvider::new(FakeScript::sample()));

    let course = harness.create_course("Tectónica de placas").await;

    assert_eq!(course.topic, "Tectónica de placas");
    assert_eq!(course.stage, CourseStage::Placement);
    assert!(course.chapters.is_empty());
    assert_eq!(course.placement, Some(state(PlacementStep::ChooseRating)));
    assert!(harness.provider.requests().is_empty());
    assert_eq!(harness.open_course(course.id).await, course);
    match harness.send(CoreRequest::ListCourses).await {
        Ok(CoreResponse::Courses { courses }) => {
            assert_eq!(courses.len(), 1);
            assert_eq!(courses[0].id, course.id);
            assert_eq!(courses[0].stage, CourseStage::Placement);
        }
        other => panic!("expected courses, got {other:?}"),
    }
}

// -- a full run ----------------------------------------------------------------

#[tokio::test]
async fn a_full_run_asks_the_rungs_the_search_dictates_and_stores_the_matching_profile() {
    let dir = tempfile::tempdir().unwrap();
    let database = database(&dir);
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder(12))])
        .answer(Task::PlacementQuestion, (1..=5).map(|n| ok(question(n))));
    let harness = start(&database, provider);
    let course = harness.create_course(TOPIC).await;
    harness.events();

    // Twelve rungs, new to this: rung 1 first. Right → 7, right → 10,
    // wrong → 8, right → 9, wrong: rungs 8 and 9 disagree after five
    // questions, so the search stops.
    let picks = [
        Pick::Right,
        Pick::Right,
        Pick::Wrong(2),
        Pick::Right,
        Pick::Wrong(3),
    ];
    let mut placement = harness
        .submit(course.id, SelfRating::NewToThis, Goal::WorkingKnowledge)
        .await;
    for (index, pick) in picks.into_iter().enumerate() {
        let question = shown(&placement);
        assert_eq!(question.number as usize, index + 1);
        assert_eq!(question.stem, stem(index + 1));
        assert_eq!(question.options.len(), 4);
        assert!(harness.requests(Task::BuildIndex).is_empty());
        placement = harness.answer(course.id, &question, pick).await;
    }

    assert_eq!(placement, state(PlacementStep::Done));
    let asked: Vec<usize> = harness
        .requests(Task::PlacementQuestion)
        .iter()
        .map(|request| asked_rung(request, 12))
        .collect();
    assert_eq!(asked, [1, 7, 10, 8, 9]);
    assert_eq!(harness.requests(Task::DraftLadder).len(), 1);
    assert_eq!(
        steps(&harness.progress(course.id)),
        expect_steps(&[
            ("PreparingQuestions", false),
            ("Question 1", false),
            ("Question 2", false),
            ("Question 3", false),
            ("Question 4", false),
            ("Question 5", false),
            ("BuildingIndex", false),
            ("Done", false),
        ])
    );

    use Knowledge::{Known, Unknown};
    let expected = profile(
        Goal::WorkingKnowledge,
        &[
            Known, Known, Known, Known, Known, Known, Known, Known, Unknown, Unknown, Unknown,
            Unknown,
        ],
        &[(10, label(3, 2)), (9, label(5, 3))],
    );
    assert_eq!(stored_profile(&database, course.id), Some(expected));

    let opened = harness.open_course(course.id).await;
    assert_eq!(opened.stage, CourseStage::Ready);
    assert_eq!(opened.placement, None);
    assert!(!opened.chapters.is_empty());
    harness.assert_nothing_leaked();
}

#[tokio::test]
async fn each_question_request_names_the_rungs_concept_and_the_goal() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder(12))])
        .answer(Task::PlacementQuestion, (1..=5).map(|n| ok(question(n))));
    let harness = start(&database(&dir), provider);
    let course = harness.create_course(TOPIC).await;

    let mut placement = harness
        .submit(course.id, SelfRating::NewToThis, Goal::WorkingKnowledge)
        .await;
    for pick in [Pick::Right, Pick::Right, Pick::Wrong(2), Pick::Right] {
        placement = harness.answer(course.id, &shown(&placement), pick).await;
    }

    let requests = harness.requests(Task::PlacementQuestion);
    assert_eq!(requests.len(), 5);
    for (request, rung) in requests.iter().zip([1, 7, 10, 8, 9]) {
        assert!(request.input.contains(&concept(rung)), "{}", request.input);
        assert!(
            request.input.to_lowercase().contains("working knowledge"),
            "{}",
            request.input
        );
    }
}

#[tokio::test]
async fn after_the_last_answer_the_index_build_is_invoked_once_with_the_profile_and_the_goal() {
    let dir = tempfile::tempdir().unwrap();
    let database = database(&dir);
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder(12))])
        .answer(Task::PlacementQuestion, (1..=5).map(|n| ok(question(n))));
    let harness = start(&database, provider);
    let course = harness.create_course(TOPIC).await;

    let mut placement = harness
        .submit(course.id, SelfRating::NewToThis, Goal::Expert)
        .await;
    let picks = [
        Pick::Right,
        Pick::Right,
        Pick::Wrong(1),
        Pick::Right,
        Pick::Wrong(3),
    ];
    for pick in picks {
        assert!(harness.requests(Task::BuildIndex).is_empty());
        placement = harness.answer(course.id, &shown(&placement), pick).await;
    }

    assert_eq!(placement.step, PlacementStep::Done);
    let builds = harness.requests(Task::BuildIndex);
    assert_eq!(builds.len(), 1);
    assert_eq!(harness.tasks().last(), Some(&Task::BuildIndex));
    let input = &builds[0].input;
    assert!(input.contains(TOPIC), "{input}");
    for rung in 1..=12 {
        assert!(input.contains(&concept(rung)), "{input}");
    }
    assert!(input.contains(&label(3, 1)), "{input}");
    assert!(input.contains(&label(5, 3)), "{input}");
    let stored = stored_profile(&database, course.id).expect("the profile is stored");
    assert_eq!(stored.goal, Goal::Expert);
    assert_eq!(stored.misconceptions.len(), 2);

    // Placement is over: a retry is refused and builds nothing.
    let refused = harness
        .send(CoreRequest::RetryPlacementStep {
            course_id: course.id,
        })
        .await;
    assert!(
        matches!(refused, Err(CoreError::InvalidRequest { .. })),
        "{refused:?}"
    );
    assert_eq!(harness.requests(Task::BuildIndex).len(), 1);
}

// -- failures and retries ------------------------------------------------------

#[tokio::test]
async fn a_ladder_of_seven_or_thirteen_concepts_is_repaired_once_then_reported_and_retry_drafts_it_again()
 {
    for len in [7, 13] {
        let dir = tempfile::tempdir().unwrap();
        let provider = FakeProvider::new(FakeScript::sample())
            .answer(Task::DraftLadder, [ok(ladder(len)), ok(ladder(10))])
            .answer(Task::RepairStructured, [ok(ladder(len))])
            .answer(Task::PlacementQuestion, [ok(question(1))]);
        let harness = start(&database(&dir), provider);
        let course = harness.create_course(TOPIC).await;
        harness.events();

        let failed = harness
            .submit(course.id, SelfRating::KnowTheBasics, Goal::Overview)
            .await;

        assert_eq!(failed.step, PlacementStep::PreparingQuestions, "{len}");
        assert!(failed.error.is_some(), "{len}");
        assert_eq!(
            harness.tasks(),
            [Task::DraftLadder, Task::RepairStructured],
            "{len}"
        );
        assert_eq!(
            steps(&harness.progress(course.id)),
            expect_steps(&[("PreparingQuestions", false), ("PreparingQuestions", true)]),
            "{len}"
        );
        let reopened = harness.open_course(course.id).await;
        assert_eq!(reopened.stage, CourseStage::Placement);

        let retried = harness.retry(course.id).await;

        assert_eq!(shown(&retried).number, 1, "{len}");
        assert_eq!(retried.error, None);
        assert_eq!(
            harness.tasks(),
            [
                Task::DraftLadder,
                Task::RepairStructured,
                Task::DraftLadder,
                Task::PlacementQuestion
            ],
            "{len}"
        );
        assert_eq!(
            steps(&harness.progress(course.id)),
            expect_steps(&[("PreparingQuestions", false), ("Question 1", false)]),
            "{len}"
        );
        // Ten rungs, know the basics: rung round(9 / 3) + 1 = 4.
        assert_eq!(
            asked_rung(&harness.requests(Task::PlacementQuestion)[0], 10),
            4
        );
        harness.assert_nothing_leaked();
    }
}

#[tokio::test]
async fn a_ladder_with_a_duplicate_concept_is_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let mut concepts: Vec<String> = (1..=10).map(concept).collect();
    concepts[6] = concept(3);
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(
            Task::DraftLadder,
            [ok(json!({ "concepts": concepts }).to_string())],
        )
        .answer(Task::RepairStructured, [ok(ladder(10))])
        .answer(Task::PlacementQuestion, [ok(question(1))]);
    let harness = start(&database(&dir), provider);
    let course = harness.create_course(TOPIC).await;

    let placement = harness
        .submit(course.id, SelfRating::NewToThis, Goal::Overview)
        .await;

    assert_eq!(shown(&placement).number, 1);
    assert_eq!(
        harness.tasks(),
        [
            Task::DraftLadder,
            Task::RepairStructured,
            Task::PlacementQuestion
        ]
    );
}

#[tokio::test]
async fn a_question_without_one_key_and_three_misconception_labels_is_repaired() {
    let dir = tempfile::tempdir().unwrap();
    // The key's option carries a label and one wrong option has none.
    let unlabelled = json!({
        "stem": stem(1),
        "options": [
            { "text": right(1), "misconception": label(1, 1) },
            { "text": wrong(1, 1), "misconception": null },
            { "text": wrong(1, 2), "misconception": label(1, 2) },
            { "text": wrong(1, 3), "misconception": label(1, 3) },
        ],
        "key": 0,
    })
    .to_string();
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder(10))])
        .answer(Task::PlacementQuestion, [ok(unlabelled)])
        .answer(Task::RepairStructured, [ok(question(1))]);
    let harness = start(&database(&dir), provider);
    let course = harness.create_course(TOPIC).await;

    let placement = harness
        .submit(course.id, SelfRating::NewToThis, Goal::Overview)
        .await;

    let question = shown(&placement);
    assert_eq!(question.stem, stem(1));
    let mut options = question.options.clone();
    options.sort();
    let mut expected = vec![right(1), wrong(1, 1), wrong(1, 2), wrong(1, 3)];
    expected.sort();
    assert_eq!(options, expected);
    assert_eq!(
        harness.tasks(),
        [
            Task::DraftLadder,
            Task::PlacementQuestion,
            Task::RepairStructured
        ]
    );
    harness.assert_nothing_leaked();
}

#[tokio::test]
async fn a_failed_question_is_reported_and_retry_writes_only_that_question() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder(10))])
        .answer(
            Task::PlacementQuestion,
            [
                ok(question(1)),
                Err(ProviderError::Offline),
                ok(question(2)),
            ],
        );
    let harness = start(&database(&dir), provider);
    let course = harness.create_course(TOPIC).await;
    let first = harness
        .submit(course.id, SelfRating::NewToThis, Goal::Overview)
        .await;
    harness.events();

    let failed = harness.answer(course.id, &shown(&first), Pick::Right).await;

    assert_eq!(failed.step, PlacementStep::PreparingQuestions);
    assert_eq!(failed.error, Some(CoreError::Offline));
    assert_eq!(
        steps(&harness.progress(course.id)),
        expect_steps(&[("PreparingQuestions", true)])
    );

    let retried = harness.retry(course.id).await;

    assert_eq!(shown(&retried).number, 2);
    assert_eq!(shown(&retried).stem, stem(2));
    assert_eq!(
        harness.tasks(),
        [
            Task::DraftLadder,
            Task::PlacementQuestion,
            Task::PlacementQuestion,
            Task::PlacementQuestion
        ]
    );
}

#[tokio::test]
async fn an_answer_to_a_question_not_shown_is_refused_and_not_stored() {
    let dir = tempfile::tempdir().unwrap();
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder(10))])
        .answer(Task::PlacementQuestion, [ok(question(1)), ok(question(2))]);
    let harness = start(&database(&dir), provider);
    let course = harness.create_course(TOPIC).await;
    let first = shown(
        &harness
            .submit(course.id, SelfRating::NewToThis, Goal::Overview)
            .await,
    );

    let stranger = harness
        .send(CoreRequest::AnswerPlacement {
            course_id: course.id,
            question_id: Uuid::new_v4(),
            answer: PlacementAnswer::NotSure,
        })
        .await;
    let fifth_option = harness
        .send(CoreRequest::AnswerPlacement {
            course_id: course.id,
            question_id: first.id,
            answer: PlacementAnswer::Option { index: 4 },
        })
        .await;

    assert!(matches!(stranger, Err(CoreError::InvalidRequest { .. })));
    assert!(matches!(
        fifth_option,
        Err(CoreError::InvalidRequest { .. })
    ));
    assert_eq!(harness.requests(Task::PlacementQuestion).len(), 1);
    let next = harness.answer(course.id, &first, Pick::NotSure).await;
    assert_eq!(shown(&next).number, 2);
}

// -- leaving and resuming ------------------------------------------------------

#[tokio::test]
async fn a_course_left_after_two_answers_resumes_at_the_third_question_with_the_answers_kept() {
    let dir = tempfile::tempdir().unwrap();
    let database = database(&dir);

    // Ten rungs, expert: rung 10 first. Right at the top, so rungs 10 and 9
    // are re-tested in turn: right on 9, wrong on 10 (now 9 and 10
    // disagree), right on 9, and the search stops after four.
    let (course, third) = {
        let provider = FakeProvider::new(FakeScript::sample())
            .answer(Task::DraftLadder, [ok(ladder(10))])
            .answer(Task::PlacementQuestion, (1..=3).map(|n| ok(question(n))));
        let harness = start(&database, provider);
        let course = harness.create_course(TOPIC).await;
        let mut placement = harness
            .submit(course.id, SelfRating::Expert, Goal::Overview)
            .await;
        for pick in [Pick::Right, Pick::Right] {
            placement = harness.answer(course.id, &shown(&placement), pick).await;
        }
        let asked: Vec<usize> = harness
            .requests(Task::PlacementQuestion)
            .iter()
            .map(|request| asked_rung(request, 10))
            .collect();
        assert_eq!(asked, [10, 9, 10]);
        harness.assert_nothing_leaked();
        (course, shown(&placement))
    };
    assert_eq!(third.number, 3);

    let provider =
        FakeProvider::new(FakeScript::sample()).answer(Task::PlacementQuestion, [ok(question(4))]);
    let harness = start(&database, provider);

    match harness.send(CoreRequest::ListCourses).await {
        Ok(CoreResponse::Courses { courses }) => {
            assert_eq!(courses.len(), 1);
            assert_eq!(courses[0].stage, CourseStage::Placement);
        }
        other => panic!("expected courses, got {other:?}"),
    }
    let opened = harness.open_course(course.id).await;
    assert_eq!(opened.stage, CourseStage::Placement);
    let placement = opened.placement.expect("the placement state");
    assert_eq!(placement.error, None);
    assert_eq!(shown(&placement), third);
    assert!(harness.provider.requests().is_empty());

    let fourth = harness.answer(course.id, &third, Pick::Wrong(1)).await;
    assert_eq!(shown(&fourth).number, 4);
    assert_eq!(shown(&fourth).stem, stem(4));
    let done = harness
        .answer(course.id, &shown(&fourth), Pick::Right)
        .await;

    assert_eq!(done.step, PlacementStep::Done);
    assert!(harness.requests(Task::DraftLadder).is_empty());
    let questions = harness.requests(Task::PlacementQuestion);
    assert_eq!(questions.len(), 1);
    assert_eq!(asked_rung(&questions[0], 10), 9);
    assert_eq!(harness.requests(Task::BuildIndex).len(), 1);
    use Knowledge::{Known, Shaky};
    let expected = profile(
        Goal::Overview,
        &[
            Known, Known, Known, Known, Known, Known, Known, Known, Known, Shaky,
        ],
        &[(10, label(3, 1))],
    );
    assert_eq!(stored_profile(&database, course.id), Some(expected));
    assert_eq!(
        harness.open_course(course.id).await.stage,
        CourseStage::Ready
    );
    harness.assert_nothing_leaked();
}
