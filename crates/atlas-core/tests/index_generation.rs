//! Building a course's index and its course map once placement is done,
//! through the core's request handler, with a scripted provider and a
//! database in a temp dir: the request sent, the answer stored in one go, the
//! repair of a map or an index that does not hold, and nothing of the map
//! leaving the core.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::{Concept, CourseMap, MapEntry};
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::profile::Knowledge;
use atlas_core::index_tree::{IndexTree, in_index_order};
use atlas_core::protocol::{
    CoreError, CoreEvent, CoreRequest, CoreResponse, CourseStage, CourseView, Goal,
    PlacementAnswer, PlacementQuestion, PlacementState, PlacementStep, SelfRating,
};
use atlas_core::provider::{FakeProvider, FakeScript, ProviderError, ProviderRequest, Task};
use atlas_core::store::Store;
use atlas_core::testing::{FakeClock, FakePorts, RecordingEvents};
use serde_json::{Value, json};
use uuid::Uuid;

const TOPIC: &str = "Plate tectonics";
const RUNGS: usize = 12;

// -- placement's script ----------------------------------------------------------

fn ladder_concept(rung: usize) -> String {
    format!("ladder concept {rung:02}")
}

fn ladder() -> String {
    let concepts: Vec<String> = (1..=RUNGS).map(ladder_concept).collect();
    json!({ "concepts": concepts }).to_string()
}

fn right(n: usize) -> String {
    format!("Right option {n}")
}

fn wrong(n: usize, k: usize) -> String {
    format!("Wrong option {n}.{k}")
}

fn label(n: usize, k: usize) -> String {
    format!("misconception label {n}.{k}")
}

/// Question `n`: four options, the right one first, each wrong one labelled.
fn question(n: usize) -> String {
    let mut options = vec![json!({ "text": right(n), "misconception": null })];
    for k in 1..=3 {
        options.push(json!({ "text": wrong(n, k), "misconception": label(n, k) }));
    }
    json!({ "stem": format!("Question {n}: what happens next?"), "options": options, "key": 0 })
        .to_string()
}

/// What the candidate picks, by question number from 1: twelve rungs, new to
/// this, so the search stops after five questions with rungs known, rungs
/// unknown and two misconceptions picked.
fn pick(n: usize) -> Option<usize> {
    match n {
        3 => Some(2),
        5 => Some(3),
        _ => None,
    }
}

// -- the index's script ----------------------------------------------------------

/// A concept of the map: a sentinel no event or response may carry.
fn concept(name: &str) -> String {
    format!("mapzq concept {name}")
}

/// Sentinels of the conventions.
fn convention_key(name: &str) -> String {
    format!("mapzq key {name}")
}

fn convention_meaning(name: &str) -> String {
    format!("mapzq meaning {name}")
}

fn section(title: &str, introduces: &[&str], relies_on: &[&str]) -> Value {
    json!({
        "title": title,
        "summary": format!("What {title} covers."),
        "introduces": introduces.iter().map(|name| concept(name)).collect::<Vec<_>>(),
        "relies_on": relies_on.iter().map(|name| concept(name)).collect::<Vec<_>>(),
    })
}

fn conventions() -> Value {
    json!([
        { "kind": "notation", "key": convention_key("v"), "meaning": convention_meaning("v") },
        { "kind": "terminology", "key": convention_key("plate"), "meaning": convention_meaning("plate") },
        { "kind": "running_example", "key": convention_key("andes"), "meaning": convention_meaning("andes") },
    ])
}

fn concepts() -> Value {
    json!([
        { "name": concept("crust"), "rung": 1 },
        { "name": concept("mantle"), "rung": null },
        { "name": concept("boundary"), "rung": null },
        { "name": concept("convection"), "rung": 3 },
        { "name": concept("subduction"), "rung": null },
    ])
}

/// A valid answer: two chapters of two sections, every concept relied on
/// introduced earlier.
fn valid() -> Value {
    json!({
        "chapters": [
            { "title": "Plates", "sections": [
                section("Crust and mantle", &["crust", "mantle"], &[]),
                section("Plate boundaries", &["boundary"], &["crust"]),
            ]},
            { "title": "Motion", "sections": [
                section("Convection", &["convection"], &["mantle"]),
                section("Subduction", &["subduction"], &["boundary", "convection"]),
            ]},
        ],
        "concepts": concepts(),
        "conventions": conventions(),
    })
}

/// "Plate boundaries" relies on subduction, which only a later section
/// introduces.
fn relies_on_a_later_concept() -> Value {
    json!({
        "chapters": [
            { "title": "Plates", "sections": [
                section("Crust and mantle", &["crust", "mantle"], &[]),
                section("Plate boundaries", &["boundary"], &["crust", "subduction"]),
            ]},
            { "title": "Motion", "sections": [
                section("Convection", &["convection"], &["mantle"]),
                section("Subduction", &["subduction"], &["boundary", "convection"]),
            ]},
        ],
        "concepts": concepts(),
        "conventions": conventions(),
    })
}

fn one_chapter() -> Value {
    json!({
        "chapters": [
            { "title": "Plates", "sections": [
                section("Crust and mantle", &["crust", "mantle"], &[]),
                section("Plate boundaries", &["boundary"], &["crust"]),
            ]},
        ],
        "concepts": concepts(),
        "conventions": conventions(),
    })
}

fn empty_chapter() -> Value {
    json!({
        "chapters": [
            { "title": "Plates", "sections": [
                section("Crust and mantle", &["crust", "mantle"], &[]),
                section("Plate boundaries", &["boundary"], &["crust"]),
            ]},
            { "title": "Motion", "sections": [] },
        ],
        "concepts": concepts(),
        "conventions": conventions(),
    })
}

fn ok(value: impl ToString) -> Result<String, ProviderError> {
    Ok(value.to_string())
}

/// Placement's answers followed by `index` for build-index and `repair` for
/// repair requests.
fn provider(index: Vec<Value>, repair: Vec<Value>) -> FakeProvider {
    FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder())])
        .answer(Task::PlacementQuestion, (1..=6).map(|n| ok(question(n))))
        .answer(Task::BuildIndex, index.into_iter().map(ok))
        .answer(Task::RepairStructured, repair.into_iter().map(ok))
}

// -- the harness -----------------------------------------------------------------

struct Harness {
    core: Core,
    database: PathBuf,
    provider: Arc<FakeProvider>,
    events: Arc<RecordingEvents>,
    /// Every response, error and event so far, serialised.
    seen: Mutex<Vec<String>>,
}

fn start(dir: &tempfile::TempDir, provider: FakeProvider) -> Harness {
    let database = dir.path().join("atlas.sqlite3");
    let provider = Arc::new(provider);
    let events = Arc::new(RecordingEvents::default());
    let core =
        Core::new(FakePorts::default().deps(database.clone(), provider.clone(), events.clone()))
            .expect("core starts");
    Harness {
        core,
        database,
        provider,
        events,
        seen: Mutex::new(Vec::new()),
    }
}

fn store(database: &Path) -> Store {
    Store::open(database, Arc::new(FakeClock::default())).unwrap()
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

    async fn course(&self, request: CoreRequest) -> CourseView {
        match self.send(request).await {
            Ok(CoreResponse::Course { course }) => course,
            other => panic!("expected a course, got {other:?}"),
        }
    }

    async fn placement(&self, request: CoreRequest) -> PlacementState {
        match self.send(request).await {
            Ok(CoreResponse::Placement { placement, .. }) => placement,
            other => panic!("expected a placement state, got {other:?}"),
        }
    }

    /// Creates a course and answers placement until it stops asking; the
    /// index is built after the last answer.
    async fn place(&self) -> (Uuid, PlacementState) {
        let course_id = self
            .course(CoreRequest::CreateCourse {
                topic: TOPIC.to_owned(),
            })
            .await
            .id;
        let mut placement = self
            .placement(CoreRequest::SubmitSelfRating {
                course_id,
                rating: SelfRating::NewToThis,
                goal: Goal::WorkingKnowledge,
            })
            .await;
        while let PlacementStep::Question { question } = &placement.step {
            let answer = answer(question);
            placement = self
                .placement(CoreRequest::AnswerPlacement {
                    course_id,
                    question_id: question.id,
                    answer,
                })
                .await;
        }
        (course_id, placement)
    }

    fn requests(&self) -> Vec<ProviderRequest> {
        self.provider.requests()
    }

    /// The requests from the first build-index request on.
    fn build_requests(&self) -> Vec<ProviderRequest> {
        let requests = self.requests();
        let first = requests
            .iter()
            .position(|request| request.task == Task::BuildIndex)
            .unwrap_or(requests.len());
        requests[first..].to_vec()
    }

    fn events(&self) -> Vec<CoreEvent> {
        let events = self.events.take();
        self.seen.lock().unwrap().extend(
            events
                .iter()
                .map(|event| serde_json::to_string(event).unwrap()),
        );
        events
    }

    /// Fails if any response, error or event carries a concept of the map
    /// or a convention.
    fn assert_map_not_leaked(&self) {
        self.events();
        for text in self.seen.lock().unwrap().iter() {
            assert!(!text.contains("mapzq"), "the map leaked in {text}");
        }
    }

    fn tree(&self, course: Uuid) -> IndexTree {
        store(&self.database).index_tree(course).unwrap()
    }
}

fn answer(question: &PlacementQuestion) -> PlacementAnswer {
    let n = question.number as usize;
    let text = match pick(n) {
        None => right(n),
        Some(k) => wrong(n, k),
    };
    let index = question
        .options
        .iter()
        .position(|option| *option == text)
        .unwrap_or_else(|| panic!("{text:?} is not among {:?}", question.options));
    PlacementAnswer::Option {
        index: index as u32,
    }
}

/// Chapter titles with their sections' titles and lines.
fn outline(tree: &IndexTree) -> Vec<(String, Vec<(String, String)>)> {
    tree.chapters
        .iter()
        .map(|chapter| {
            (
                chapter.title.clone(),
                chapter
                    .sections
                    .iter()
                    .map(|section| (section.title.clone(), section.line.clone()))
                    .collect(),
            )
        })
        .collect()
}

fn valid_outline() -> Vec<(String, Vec<(String, String)>)> {
    let section = |title: &str| (title.to_owned(), format!("What {title} covers."));
    vec![
        (
            "Plates".to_owned(),
            vec![section("Crust and mantle"), section("Plate boundaries")],
        ),
        (
            "Motion".to_owned(),
            vec![section("Convection"), section("Subduction")],
        ),
    ]
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| concept(name)).collect()
}

/// The map of [`valid`], on the stored section ids in index order.
fn valid_map(sections: &[Uuid]) -> CourseMap {
    let entry = |at: usize, introduces: &[&str], relies_on: &[&str]| MapEntry {
        section: sections[at],
        introduces: names(introduces),
        relies_on: names(relies_on),
    };
    CourseMap {
        concepts: vec![
            Concept {
                name: concept("crust"),
                rung: Some(RungId(1)),
            },
            Concept {
                name: concept("mantle"),
                rung: None,
            },
            Concept {
                name: concept("boundary"),
                rung: None,
            },
            Concept {
                name: concept("convection"),
                rung: Some(RungId(3)),
            },
            Concept {
                name: concept("subduction"),
                rung: None,
            },
        ],
        entries: vec![
            entry(0, &["crust", "mantle"], &[]),
            entry(1, &["boundary"], &["crust"]),
            entry(2, &["convection"], &["mantle"]),
            entry(3, &["subduction"], &["boundary", "convection"]),
        ],
    }
}

fn valid_conventions() -> Vec<Convention> {
    let convention = |kind, name: &str| Convention {
        kind,
        key: convention_key(name),
        meaning: convention_meaning(name),
        origin: ConventionOrigin::Plan,
    };
    vec![
        convention(ConventionKind::Notation, "v"),
        convention(ConventionKind::Terminology, "plate"),
        convention(ConventionKind::RunningExample, "andes"),
    ]
}

fn section_ids(tree: &IndexTree) -> Vec<Uuid> {
    in_index_order(tree)
        .iter()
        .map(|entry| entry.section.id)
        .collect()
}

/// Asserts the course holds the index, map and conventions of [`valid`],
/// is ready and resumes at its first section.
async fn assert_valid_stored(harness: &Harness, course: Uuid) {
    let tree = harness.tree(course);
    assert_eq!(outline(&tree), valid_outline());
    let in_order: Vec<String> = in_index_order(&tree)
        .iter()
        .map(|entry| entry.section.title.clone())
        .collect();
    assert_eq!(
        in_order,
        [
            "Crust and mantle",
            "Plate boundaries",
            "Convection",
            "Subduction"
        ]
    );
    let store = store(&harness.database);
    assert_eq!(
        store.course_map(course).unwrap(),
        valid_map(&section_ids(&tree))
    );
    assert_eq!(store.conventions(course).unwrap(), valid_conventions());

    let opened = harness
        .course(CoreRequest::OpenCourse { course_id: course })
        .await;
    assert_eq!(opened.stage, CourseStage::Ready);
    assert_eq!(opened.resume_at, Some(tree.chapters[0].sections[0].id));
}

fn assert_nothing_stored(harness: &Harness, course: Uuid) {
    let store = store(&harness.database);
    assert!(harness.tree(course).chapters.is_empty());
    let map = store.course_map(course).unwrap();
    assert!(map.concepts.is_empty() && map.entries.is_empty(), "{map:?}");
    assert!(store.conventions(course).unwrap().is_empty());
    assert_eq!(
        store.course_record(course).unwrap().unwrap().stage,
        CourseStage::Placement
    );
}

// -- the request -----------------------------------------------------------------

#[tokio::test]
async fn the_request_names_the_topic_every_rung_with_its_id_and_state_the_misconceptions_and_the_goal()
 {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir, provider(vec![valid()], vec![]));

    let (course, placement) = harness.place().await;

    assert_eq!(placement.step, PlacementStep::Done, "{placement:?}");
    let builds = harness.build_requests();
    assert_eq!(builds.len(), 1);
    assert_eq!(builds[0].task, Task::BuildIndex);
    let input = &builds[0].input;
    assert!(input.contains(TOPIC), "{input}");
    assert!(
        input.to_lowercase().contains("working knowledge"),
        "{input}"
    );

    let profile = store(&harness.database)
        .profile(course)
        .unwrap()
        .expect("placement stored the profile");
    assert_eq!(profile.rungs.len(), RUNGS);
    assert!(profile.rungs.iter().any(|r| r.state == Knowledge::Known));
    assert!(profile.rungs.iter().any(|r| r.state == Knowledge::Unknown));
    // The index and the map are asked for in one structured answer.
    let schema = builds[0]
        .schema
        .as_ref()
        .expect("the build asks for structured output")
        .schema
        .to_string();
    for field in [
        "chapters",
        "introduces",
        "relies_on",
        "concepts",
        "rung",
        "conventions",
    ] {
        assert!(
            schema.contains(&format!("\"{field}\"")),
            "{field}: {schema}"
        );
    }
    assert_eq!(profile.misconceptions.len(), 2);
    for rung in &profile.rungs {
        let concept = ladder_concept(rung.rung.0 as usize);
        let line = input
            .lines()
            .find(|line| line.contains(&concept))
            .unwrap_or_else(|| panic!("{concept:?} is not in {input}"));
        assert!(line.contains(&format!("rung {} ", rung.rung.0)), "{line}");
        let state = match rung.state {
            Knowledge::Known => "known",
            Knowledge::Shaky => "shaky",
            Knowledge::Unknown => "unknown",
        };
        assert!(line.contains(state), "{line}");
    }
    for misconception in &profile.misconceptions {
        assert!(input.contains(&misconception.text), "{input}");
    }
}

// -- a valid answer --------------------------------------------------------------

#[tokio::test]
async fn a_valid_answer_is_stored_as_the_index_the_map_and_the_conventions_and_the_course_is_ready()
{
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir, provider(vec![valid()], vec![]));

    let (course, placement) = harness.place().await;

    assert_eq!(placement.step, PlacementStep::Done, "{placement:?}");
    assert_eq!(harness.build_requests().len(), 1);
    assert_valid_stored(&harness, course).await;

    let tree = harness.tree(course);
    let changed: Vec<_> = harness
        .events()
        .into_iter()
        .filter_map(|event| match event {
            CoreEvent::IndexChanged {
                course_id,
                chapters,
            } if course_id == course => Some(chapters),
            _ => None,
        })
        .collect();
    assert_eq!(changed.len(), 1, "{changed:?}");
    let titles: Vec<(Uuid, String, Vec<(Uuid, String)>)> = changed[0]
        .iter()
        .map(|chapter| {
            (
                chapter.id,
                chapter.title.clone(),
                chapter
                    .sections
                    .iter()
                    .map(|section| (section.id, section.title.clone()))
                    .collect(),
            )
        })
        .collect();
    let stored: Vec<(Uuid, String, Vec<(Uuid, String)>)> = tree
        .chapters
        .iter()
        .map(|chapter| {
            (
                chapter.id,
                chapter.title.clone(),
                chapter
                    .sections
                    .iter()
                    .map(|section| (section.id, section.title.clone()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(titles, stored);
    harness.assert_map_not_leaked();
}

// -- repairs ---------------------------------------------------------------------

#[tokio::test]
async fn a_section_relying_on_a_concept_no_earlier_section_introduces_is_repaired_with_that_section_and_concept_named()
 {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(
        &dir,
        provider(vec![relies_on_a_later_concept()], vec![valid()]),
    );

    let (course, placement) = harness.place().await;

    assert_eq!(placement.step, PlacementStep::Done, "{placement:?}");
    let builds = harness.build_requests();
    let tasks: Vec<Task> = builds.iter().map(|request| request.task).collect();
    assert_eq!(tasks, [Task::BuildIndex, Task::RepairStructured]);
    let repair = &builds[1].input;
    let named = repair
        .lines()
        .any(|line| line.contains("Plate boundaries") && line.contains(&concept("subduction")));
    assert!(named, "{repair}");
    assert_valid_stored(&harness, course).await;
    harness.assert_map_not_leaked();
}

#[tokio::test]
async fn the_same_fault_twice_is_reported_after_two_requests_and_nothing_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(
        &dir,
        provider(
            vec![relies_on_a_later_concept()],
            vec![relies_on_a_later_concept()],
        ),
    );

    let (course, placement) = harness.place().await;

    assert_eq!(placement.step, PlacementStep::BuildingIndex);
    assert!(placement.error.is_some(), "{placement:?}");
    let tasks: Vec<Task> = harness
        .build_requests()
        .iter()
        .map(|request| request.task)
        .collect();
    assert_eq!(tasks, [Task::BuildIndex, Task::RepairStructured]);
    assert_nothing_stored(&harness, course);
    assert!(
        !harness.events().into_iter().any(|event| matches!(
            event,
            CoreEvent::IndexChanged { course_id, .. } if course_id == course
        )),
        "no index-changed event after a failed build"
    );
    harness.assert_map_not_leaked();
}

#[tokio::test]
async fn an_answer_with_one_chapter_or_an_empty_chapter_is_repaired() {
    for faulty in [one_chapter(), empty_chapter()] {
        let dir = tempfile::tempdir().unwrap();
        let harness = start(&dir, provider(vec![faulty.clone()], vec![valid()]));

        let (course, placement) = harness.place().await;

        assert_eq!(
            placement.step,
            PlacementStep::Done,
            "{faulty}: {placement:?}"
        );
        let tasks: Vec<Task> = harness
            .build_requests()
            .iter()
            .map(|request| request.task)
            .collect();
        assert_eq!(
            tasks,
            [Task::BuildIndex, Task::RepairStructured],
            "{faulty}"
        );
        assert_valid_stored(&harness, course).await;
    }
}

#[tokio::test]
async fn an_answer_with_one_chapter_or_an_empty_chapter_twice_is_reported_and_nothing_is_stored() {
    for faulty in [one_chapter(), empty_chapter()] {
        let dir = tempfile::tempdir().unwrap();
        let harness = start(&dir, provider(vec![faulty.clone()], vec![faulty.clone()]));

        let (course, placement) = harness.place().await;

        assert_eq!(placement.step, PlacementStep::BuildingIndex, "{faulty}");
        assert!(placement.error.is_some(), "{faulty}: {placement:?}");
        assert_eq!(harness.build_requests().len(), 2, "{faulty}");
        assert_nothing_stored(&harness, course);
    }
}

// -- building once ---------------------------------------------------------------

#[tokio::test]
async fn building_a_course_that_has_an_index_makes_no_request_and_keeps_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir, provider(vec![valid(), valid()], vec![]));
    let (course, placement) = harness.place().await;
    assert_eq!(placement.step, PlacementStep::Done, "{placement:?}");
    let built = harness.tree(course);

    // Placement's build runs again for a course that already has its index.
    store(&harness.database)
        .set_stage(course, CourseStage::Placement)
        .unwrap();
    let again = harness
        .placement(CoreRequest::RetryPlacementStep { course_id: course })
        .await;

    assert_eq!(again.step, PlacementStep::Done, "{again:?}");
    let builds: Vec<Task> = harness
        .build_requests()
        .iter()
        .map(|request| request.task)
        .collect();
    assert_eq!(builds, [Task::BuildIndex]);
    assert_eq!(harness.tree(course), built);
    assert_valid_stored(&harness, course).await;
}
