//! The library and Next through the core's request handler, on a database in
//! a temp dir with a fake clock.
//!
//! Courses are laid down through the store's repository API on the same file
//! (creating one through the core needs a plan login and a placement run).
//! Raw SQL only looks under the API: the rows a deletion leaves, and whether
//! anything of a deleted course can still be read from the file.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::{Concept, CourseMap, MapEntry};
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::questions::{KeyedOption, KeyedPlacementQuestion};
use atlas_core::domain::record::{RecordStatus, SectionRecord, StoredRecord, TaughtConcept};
use atlas_core::index_tree::{IndexTree, TreeChapter, TreeSection};
use atlas_core::ports::Clock;
use atlas_core::protocol::{
    Anchor, Block, BlockContent, ChapterNode, ChatRole, CoreError, CoreEvent, CoreRequest,
    CoreResponse, CourseStage, CourseSummary, CourseView, MessageState, NodeMark, PlacementAnswer,
    PlacementStep, Progress, SectionKind, SectionNode, SectionView, VersionStyle,
};
use atlas_core::provider::{FakeProvider, FakeScript, Task};
use atlas_core::store::{NewChatMessage, NewGlossaryEntry, NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, RecordingEvents};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use rusqlite::types::Value;
use support::sample_course::{
    CHAPTER_BOUNDARIES, CHAPTER_MOTION, CHAPTER_STRUCTURE, HOW_PLATES_ARE_MAPPED,
    KINDS_OF_BOUNDARY, LAYERS, LAYERS_V1, LITHOSPHERE, MICROPLATES, PLATE_THICKNESS,
    SUBDUCTION_ZONES, SampleCourse, WHAT_A_PLATE_IS, WHAT_DRIVES_PLATES, sample_course,
};
use tempfile::TempDir;
use uuid::Uuid;

use async_trait::async_trait;
use atlas_core::protocol::Model;
use atlas_core::provider::{ModelProvider, ProviderError, ProviderRequest, ProviderStream};
use atlas_core::store::RecordKind;
use tokio::sync::Notify;

// Strings stored once each in the course that is deleted.
const SECTION_SENTINEL: &str = "sentinel-in-a-section-4c1d";
const RECORD_SENTINEL: &str = "sentinel-in-a-record-9e27";
const CONVENTION_SENTINEL: &str = "sentinel-in-a-convention-b06a";
const MESSAGE_SENTINEL: &str = "sentinel-in-a-message-73f5";

struct Harness {
    _dir: TempDir,
    path: PathBuf,
    ports: FakePorts,
    provider: Arc<FakeProvider>,
    events: Arc<RecordingEvents>,
    core: Core,
    /// The same database file, for laying courses down.
    store: Store,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(FakeProvider::new(FakeScript::sample()));
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        Self {
            _dir: dir,
            path,
            ports,
            provider,
            events,
            core,
            store,
        }
    }

    /// A harness whose core asks `provider`.
    fn answering(provider: FakeProvider) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(provider);
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        Self {
            _dir: dir,
            path,
            ports,
            provider,
            events,
            core,
            store,
        }
    }

    fn raw(&self) -> Connection {
        Connection::open(&self.path).unwrap()
    }

    /// Moves the clock on by an hour and returns the new time.
    fn advance(&self) -> DateTime<Utc> {
        self.ports.clock.advance(Duration::from_secs(3600));
        self.ports.clock.now()
    }

    async fn send(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        self.core.handle(request).await
    }

    async fn courses(&self) -> Vec<CourseSummary> {
        match self.send(CoreRequest::ListCourses).await {
            Ok(CoreResponse::Courses { courses }) => courses,
            other => panic!("expected the courses, got {other:?}"),
        }
    }

    async fn summary(&self, course_id: Uuid) -> CourseSummary {
        self.courses()
            .await
            .into_iter()
            .find(|course| course.id == course_id)
            .unwrap_or_else(|| panic!("course {course_id} is listed"))
    }

    async fn open(&self, course_id: Uuid) -> CourseView {
        match self.send(CoreRequest::OpenCourse { course_id }).await {
            Ok(CoreResponse::Course { course }) => course,
            other => panic!("expected a course, got {other:?}"),
        }
    }

    async fn open_section(&self, course_id: Uuid, section_id: Uuid) -> SectionView {
        match self
            .send(CoreRequest::OpenSection {
                course_id,
                section_id,
            })
            .await
        {
            Ok(CoreResponse::Section { section }) => section,
            other => panic!("expected a section, got {other:?}"),
        }
    }

    async fn next(&self, course_id: Uuid, section_id: Uuid) -> Result<CoreResponse, CoreError> {
        self.send(CoreRequest::Next {
            course_id,
            section_id,
        })
        .await
    }
}

// -- laying courses down ---------------------------------------------------------

fn prose(text: &str) -> Vec<Block> {
    vec![Block {
        index: 0,
        content: BlockContent::Prose {
            text: text.to_owned(),
        },
    }]
}

fn new_version(id: Uuid, node: Uuid, text: &str, record: StoredRecord) -> NewSectionVersion {
    NewSectionVersion {
        id,
        node,
        style: VersionStyle::Original,
        blocks: prose(text),
        checks: Vec::new(),
        citations: Vec::new(),
        sources: Vec::new(),
        written_without_search: false,
        record,
        model: Some("gpt-5".to_owned()),
        prompt: "write-section@1".to_owned(),
    }
}

fn anchor(version: Uuid, quote: &str) -> Anchor {
    Anchor {
        version_id: version,
        block: 0,
        quote: quote.to_owned(),
    }
}

/// Stores a course: ladder, profile, index with its map and the plan's
/// conventions, then each record's version with the conventions it fixed.
/// The version of `LAYERS` gets `section_text` when given.
fn store_course(store: &Store, sample: &SampleCourse, section_text: Option<&str>) -> Uuid {
    let course = store.create_course(&sample.topic).unwrap().id;
    store.store_ladder(course, &sample.ladder).unwrap();
    store.save_profile(course, &sample.profile).unwrap();
    let plan: Vec<Convention> = sample
        .conventions
        .iter()
        .filter(|convention| convention.origin == ConventionOrigin::Plan)
        .cloned()
        .collect();
    store
        .insert_index(course, &sample.index, &sample.map, &plan)
        .unwrap();
    for record in &sample.records {
        let fixed: Vec<Convention> = sample
            .conventions
            .iter()
            .filter(|convention| {
                convention.origin
                    == ConventionOrigin::SectionVersion {
                        version: record.version,
                    }
            })
            .cloned()
            .collect();
        let text = match section_text {
            Some(text) if record.section == LAYERS => text.to_owned(),
            _ => format!("The text of version {}.", record.version),
        };
        store
            .insert_version(
                &new_version(record.version, record.section, &text, record.record.clone()),
                &fixed,
            )
            .unwrap();
    }
    course
}

/// Adds to a course a placement question with its answer, a check answer, a
/// glossary entry, and a side chat with two messages, all on `version`.
fn furnish(store: &Store, course: Uuid, version: Uuid, message: &str) {
    let question = KeyedPlacementQuestion {
        id: Uuid::new_v4(),
        rung: RungId(1),
        stem: "What lies under the crust?".to_owned(),
        options: (0..4)
            .map(|option| KeyedOption {
                text: format!("Option {option}"),
                misconception: (option != 0).then(|| format!("Mistake {option}")),
            })
            .collect(),
        key: 0,
    };
    store.add_placement_question(course, &question).unwrap();
    store
        .answer_placement_question(question.id, PlacementAnswer::Option { index: 2 })
        .unwrap();
    store.add_check_answer(version, 1, 0, 2, false).unwrap();
    store
        .add_glossary_entry(&NewGlossaryEntry {
            course,
            term: "crust".to_owned(),
            definition: "The thin rocky outer layer.".to_owned(),
            anchor: anchor(version, "crust"),
        })
        .unwrap();
    let chat = store
        .start_side_chat(course, Some(&anchor(version, "crust")))
        .unwrap();
    for (role, text) in [
        (ChatRole::Candidate, message.to_owned()),
        (ChatRole::Agent, "Because it is hot.".to_owned()),
    ] {
        store
            .add_chat_message(
                chat.id,
                &NewChatMessage {
                    role,
                    text,
                    state: MessageState::Complete,
                    citations: Vec::new(),
                    sources: Vec::new(),
                },
            )
            .unwrap();
    }
}

/// The sample course with a sentinel in a section, a record, a convention
/// and a side-chat message, and rows in every table.
fn store_doomed_course(store: &Store) -> Uuid {
    let mut sample = sample_course();
    let layers = sample
        .records
        .iter_mut()
        .find(|record| record.section == LAYERS)
        .unwrap();
    layers.record.record.summary = Some(format!("The Earth is layered. {RECORD_SENTINEL}"));
    let course = store_course(
        store,
        &sample,
        Some(&format!("Crust, mantle and core. {SECTION_SENTINEL}")),
    );
    store
        .add_convention(
            course,
            &Convention {
                kind: ConventionKind::RunningExample,
                key: "the Pacific".to_owned(),
                meaning: format!("the shrinking Pacific Ocean {CONVENTION_SENTINEL}"),
                origin: ConventionOrigin::Plan,
            },
        )
        .unwrap();
    furnish(
        store,
        course,
        LAYERS_V1,
        &format!("Why does the mantle flow? {MESSAGE_SENTINEL}"),
    );
    course
}

/// A small course with its own ids: chapters [[first, second], [third]].
struct SmallCourse {
    id: Uuid,
    sections: [Uuid; 3],
}

fn small_tree(sections: [Uuid; 3], read: [bool; 3]) -> IndexTree {
    let section = |at: usize, title: &str| TreeSection {
        id: sections[at],
        title: title.to_owned(),
        line: format!("What {title} covers."),
        read: read[at],
        children: Vec::new(),
    };
    IndexTree {
        chapters: vec![
            TreeChapter {
                id: Uuid::new_v4(),
                title: "Volcanoes".to_owned(),
                sections: vec![section(0, "What a volcano is"), section(1, "Magma")],
            },
            TreeChapter {
                id: Uuid::new_v4(),
                title: "Eruptions".to_owned(),
                sections: vec![section(2, "Kinds of eruption")],
            },
        ],
    }
}

/// A course on volcanoes with an index of three sections, none read.
/// `furnished` adds a read first section with its version, a convention, a
/// placement answer, a check answer, a glossary entry and a side chat.
fn store_small_course(store: &Store, topic: &str, furnished: bool) -> SmallCourse {
    let sections = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let sample = sample_course();
    let course = store.create_course(topic).unwrap().id;
    store.store_ladder(course, &sample.ladder).unwrap();
    store.save_profile(course, &sample.profile).unwrap();
    let concept = |name: &str| Concept {
        name: name.to_owned(),
        rung: None,
    };
    let entry = |section: Uuid, introduces: &str, relies_on: &[&str]| MapEntry {
        section,
        introduces: vec![introduces.to_owned()],
        relies_on: relies_on.iter().map(|&name| name.to_owned()).collect(),
    };
    let map = CourseMap {
        concepts: vec![concept("volcano"), concept("magma"), concept("eruption")],
        entries: vec![
            entry(sections[0], "volcano", &[]),
            entry(sections[1], "magma", &["volcano"]),
            entry(sections[2], "eruption", &["magma"]),
        ],
    };
    let conventions = [Convention {
        kind: ConventionKind::Terminology,
        key: "vent".to_owned(),
        meaning: "the opening magma escapes through".to_owned(),
        origin: ConventionOrigin::Plan,
    }];
    store
        .insert_index(
            course,
            &small_tree(sections, [furnished, false, false]),
            &map,
            &conventions,
        )
        .unwrap();
    if furnished {
        let version = Uuid::new_v4();
        let record = StoredRecord {
            record: SectionRecord {
                summary: Some("A volcano is an opening in the crust.".to_owned()),
                concepts: vec![TaughtConcept {
                    name: "volcano".to_owned(),
                    line: Some("An opening magma escapes through.".to_owned()),
                }],
                notation: Vec::new(),
                terms: Vec::new(),
                examples: Vec::new(),
                promises: Vec::new(),
            },
            status: RecordStatus::Confirmed,
        };
        store
            .insert_version(
                &new_version(version, sections[0], "Volcanoes are vents.", record),
                &[],
            )
            .unwrap();
        furnish(store, course, version, "What is a vent?");
    }
    SmallCourse {
        id: course,
        sections,
    }
}

// -- looking under the API -------------------------------------------------------

/// The tables that hold a course's syncable records.
const TABLES: [&str; 15] = [
    "courses",
    "ladder_rungs",
    "placement_questions",
    "profile_rungs",
    "misconceptions",
    "index_nodes",
    "section_versions",
    "check_answers",
    "concepts",
    "map_entries",
    "map_entry_concepts",
    "conventions",
    "glossary_entries",
    "side_chats",
    "side_chat_messages",
];

/// The rows of `table` that belong to the course `?1`.
fn belongs(table: &str) -> &'static str {
    match table {
        "courses" => "id = ?1",
        "section_versions" => "node_id IN (SELECT id FROM index_nodes WHERE course_id = ?1)",
        "check_answers" => {
            "version_id IN (SELECT version.id FROM section_versions version
                            JOIN index_nodes node ON node.id = version.node_id
                            WHERE node.course_id = ?1)"
        }
        "map_entry_concepts" => "entry_id IN (SELECT id FROM map_entries WHERE course_id = ?1)",
        "side_chat_messages" => "chat_id IN (SELECT id FROM side_chats WHERE course_id = ?1)",
        _ => "course_id = ?1",
    }
}

/// The columns of `table` that hold what was written, said or chosen: empty
/// (NULL or '') once the row is deleted.
fn content(table: &str) -> &'static [&'static str] {
    match table {
        "courses" => &["topic", "self_rating", "goal"],
        "ladder_rungs" => &["concept"],
        "placement_questions" => &["question", "answer"],
        "misconceptions" => &["text"],
        "index_nodes" => &["title", "summary"],
        "section_versions" => &["blocks", "checks", "citations", "sources", "record"],
        "concepts" => &["name"],
        "conventions" => &["key", "normalised_key", "meaning"],
        "glossary_entries" => &["quote", "term", "definition", "wikipedia"],
        "side_chats" => &["quote"],
        "side_chat_messages" => &["text", "citations", "sources"],
        _ => &[],
    }
}

type RowKey = (String, String);
type Rows = BTreeMap<RowKey, BTreeMap<String, Value>>;

/// Every row of `tables`, keyed by table and record id (by position in a
/// table without ids), column by column.
// `column_names` already yields an iterator; the `into_iter` is redundant.
#[allow(clippy::useless_conversion)]
fn dump(connection: &Connection, tables: &[String]) -> Rows {
    let mut rows = Rows::new();
    for table in tables {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table}"))
            .unwrap();
        let columns: Vec<String> = statement
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let mut cursor = statement.query([]).unwrap();
        let mut position = 0;
        while let Some(row) = cursor.next().unwrap() {
            let values: BTreeMap<String, Value> = columns
                .iter()
                .enumerate()
                .map(|(index, column)| (column.clone(), row.get::<_, Value>(index).unwrap()))
                .collect();
            let key = match values.get("id") {
                Some(Value::Text(id)) => id.clone(),
                _ => format!("#{position}"),
            };
            position += 1;
            rows.insert((table.clone(), key), values);
        }
    }
    rows
}

fn every_table(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn course_tables() -> Vec<String> {
    TABLES.iter().map(|&table| table.to_owned()).collect()
}

/// The rows that belong to `course`, by table and record id.
fn owned_rows(connection: &Connection, course: Uuid) -> BTreeSet<RowKey> {
    let mut owned = BTreeSet::new();
    for table in TABLES {
        let mut statement = connection
            .prepare(&format!("SELECT id FROM {table} WHERE {}", belongs(table)))
            .unwrap();
        let ids: Vec<String> = statement
            .query_map([course.to_string()], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for id in ids {
            owned.insert((table.to_owned(), id));
        }
    }
    owned
}

fn time(value: &Value) -> DateTime<Utc> {
    match value {
        Value::Text(text) => DateTime::parse_from_rfc3339(text)
            .unwrap_or_else(|error| panic!("timestamp {text:?}: {error}"))
            .with_timezone(&Utc),
        other => panic!("expected a timestamp, got {other:?}"),
    }
}

fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Text(text) => text.is_empty(),
        _ => false,
    }
}

/// The sentinels found in any column of any table.
fn sentinels_in_tables(connection: &Connection) -> BTreeSet<&'static str> {
    let rows = dump(connection, &every_table(connection));
    [
        SECTION_SENTINEL,
        RECORD_SENTINEL,
        CONVENTION_SENTINEL,
        MESSAGE_SENTINEL,
    ]
    .into_iter()
    .filter(|sentinel| {
        rows.values()
            .flat_map(BTreeMap::values)
            .any(|value| match value {
                Value::Text(text) => text.contains(sentinel),
                Value::Blob(bytes) => bytes
                    .windows(sentinel.len())
                    .any(|window| window == sentinel.as_bytes()),
                _ => false,
            })
    })
    .collect()
}

// -- the library -----------------------------------------------------------------

#[tokio::test]
async fn courses_are_listed_most_recently_opened_first() {
    let harness = Harness::new();
    let oldest = harness.store.create_course("Oldest").unwrap().id;
    harness.advance();
    let middle = harness.store.create_course("Middle").unwrap().id;
    harness.advance();
    let newest = harness.store.create_course("Newest").unwrap().id;

    let opened_oldest = harness.advance();
    harness.open(oldest).await;
    let opened_middle = harness.advance();
    harness.open(middle).await;
    let opened_newest = harness.advance();
    harness.open(newest).await;

    let listed = harness.courses().await;
    let order: Vec<(Uuid, &str, Option<DateTime<Utc>>)> = listed
        .iter()
        .map(|course| (course.id, course.topic.as_str(), course.last_opened_at))
        .collect();
    assert_eq!(
        order,
        [
            (newest, "Newest", Some(opened_newest)),
            (middle, "Middle", Some(opened_middle)),
            (oldest, "Oldest", Some(opened_oldest)),
        ]
    );

    let reopened = harness.advance();
    harness.open(oldest).await;

    let listed = harness.courses().await;
    let order: Vec<(Uuid, Option<DateTime<Utc>>)> = listed
        .iter()
        .map(|course| (course.id, course.last_opened_at))
        .collect();
    assert_eq!(
        order,
        [
            (oldest, Some(reopened)),
            (newest, Some(opened_newest)),
            (middle, Some(opened_middle)),
        ]
    );
}

#[tokio::test]
async fn a_course_lists_its_progress_stage_and_where_it_resumes() {
    let harness = Harness::new();
    let sample = store_course(&harness.store, &sample_course(), None);
    let fresh = store_small_course(&harness.store, "Volcanoes", false);
    let placing = harness.store.create_course("Glaciers").unwrap().id;

    let listed = harness.courses().await;
    assert_eq!(listed.len(), 3);

    // Nine sections present (two, three with a deep dive of three, two);
    // four read: both of chapter one, "What a plate is" and its first child.
    let summary = harness.summary(sample).await;
    assert_eq!(summary.topic, "Plate tectonics");
    assert_eq!(summary.stage, CourseStage::Ready);
    assert_eq!(summary.progress, Progress { read: 4, total: 9 });
    assert_eq!(summary.last_opened_at, None);
    assert_eq!(summary.resume_title.as_deref(), Some("Layers of the Earth"));

    let summary = harness.summary(fresh.id).await;
    assert_eq!(summary.stage, CourseStage::Ready);
    assert_eq!(summary.progress, Progress { read: 0, total: 3 });
    assert_eq!(summary.resume_title.as_deref(), Some("What a volcano is"));

    let summary = harness.summary(placing).await;
    assert_eq!(summary.topic, "Glaciers");
    assert_eq!(summary.stage, CourseStage::Placement);
    assert_eq!(summary.progress, Progress { read: 0, total: 0 });
    assert_eq!(summary.resume_title, None);
}

#[tokio::test]
async fn a_course_resumes_at_the_section_last_opened() {
    let harness = Harness::new();
    let course = store_course(&harness.store, &sample_course(), None);

    harness.open_section(course, LITHOSPHERE).await;
    harness.open_section(course, HOW_PLATES_ARE_MAPPED).await;
    assert_eq!(
        harness.summary(course).await.resume_title.as_deref(),
        Some("How plates are mapped")
    );
    assert_eq!(
        harness.open(course).await.resume_at,
        Some(HOW_PLATES_ARE_MAPPED)
    );

    // An earlier section opened last is where it resumes, not the furthest.
    harness.open_section(course, LAYERS).await;
    assert_eq!(
        harness.summary(course).await.resume_title.as_deref(),
        Some("Layers of the Earth")
    );
    assert_eq!(harness.open(course).await.resume_at, Some(LAYERS));
    assert_eq!(
        harness.summary(course).await.progress,
        Progress { read: 4, total: 9 }
    );
}

#[tokio::test]
async fn a_section_whose_generation_failed_is_not_where_the_course_resumes() {
    let harness = Harness::answering(
        FakeProvider::new(FakeScript::sample())
            .answer(Task::WriteSection, [Err(ProviderError::Offline)]),
    );
    let course = store_course(&harness.store, &sample_course(), None);
    harness.open_section(course, LITHOSPHERE).await;

    // The next section has no version yet; its generation fails, so it was
    // never read.
    let failed = harness
        .send(CoreRequest::OpenSection {
            course_id: course,
            section_id: WHAT_DRIVES_PLATES,
        })
        .await;
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(harness.provider.requests().len(), 1, "it was generated");

    let summary = harness.summary(course).await;
    assert_eq!(summary.progress, Progress { read: 4, total: 9 });
    assert_eq!(
        summary.resume_title.as_deref(),
        Some("Lithosphere and asthenosphere")
    );
    assert_eq!(harness.open(course).await.resume_at, Some(LITHOSPHERE));
}

#[tokio::test]
async fn a_section_that_finished_generating_is_where_the_course_resumes() {
    let harness = Harness::new();
    let course = store_course(&harness.store, &sample_course(), None);
    harness.open_section(course, LITHOSPHERE).await;

    harness.open_section(course, WHAT_DRIVES_PLATES).await;

    let summary = harness.summary(course).await;
    assert_eq!(summary.progress, Progress { read: 5, total: 9 });
    assert_eq!(
        summary.resume_title.as_deref(),
        Some("What drives the plates")
    );
    assert_eq!(
        harness.open(course).await.resume_at,
        Some(WHAT_DRIVES_PLATES)
    );
}

fn node(
    id: Uuid,
    kind: SectionKind,
    title: &str,
    summary: &str,
    mark: NodeMark,
    deep_dives: Vec<SectionNode>,
) -> SectionNode {
    SectionNode {
        id,
        kind,
        title: title.to_owned(),
        summary: summary.to_owned(),
        read: mark != NodeMark::Unread,
        mark,
        deep_dives,
    }
}

#[tokio::test]
async fn opening_a_course_returns_its_index_with_reading_states_counts_and_resume_point() {
    let harness = Harness::new();
    let course = store_course(&harness.store, &sample_course(), None);
    furnish(
        &harness.store,
        course,
        LAYERS_V1,
        "Why does the mantle flow?",
    );
    harness
        .store
        .add_glossary_entry(&NewGlossaryEntry {
            course,
            term: "mantle".to_owned(),
            definition: "Hot rock below the crust.".to_owned(),
            anchor: anchor(LAYERS_V1, "mantle"),
        })
        .unwrap();
    let opened_at = harness.advance();

    let view = harness.open(course).await;

    use NodeMark::{Branch, Read, Unread};
    use SectionKind::{BroadPass, DeepDive};
    let expected = CourseView {
        id: course,
        topic: "Plate tectonics".to_owned(),
        stage: CourseStage::Ready,
        chapters: vec![
            ChapterNode {
                id: CHAPTER_STRUCTURE,
                title: "Earth's structure".to_owned(),
                sections: vec![
                    node(
                        LAYERS,
                        BroadPass,
                        "Layers of the Earth",
                        "Crust, mantle and core, and what sets them apart.",
                        Read,
                        vec![],
                    ),
                    node(
                        LITHOSPHERE,
                        BroadPass,
                        "Lithosphere and asthenosphere",
                        "The rigid shell and the soft layer it rides on.",
                        Read,
                        vec![],
                    ),
                ],
            },
            ChapterNode {
                id: CHAPTER_MOTION,
                title: "Plates in motion".to_owned(),
                sections: vec![
                    SectionNode {
                        read: true,
                        ..node(
                            WHAT_A_PLATE_IS,
                            BroadPass,
                            "What a plate is",
                            "A rigid piece of the lithosphere that moves as one.",
                            Branch,
                            vec![
                                node(
                                    HOW_PLATES_ARE_MAPPED,
                                    DeepDive,
                                    "How plates are mapped",
                                    "Earthquake belts and satellite positions draw the edges.",
                                    Read,
                                    vec![],
                                ),
                                node(
                                    PLATE_THICKNESS,
                                    DeepDive,
                                    "Plate thickness",
                                    "Why oceanic plates are thin and continental ones thick.",
                                    Unread,
                                    vec![],
                                ),
                                node(
                                    MICROPLATES,
                                    DeepDive,
                                    "Microplates",
                                    "Small plates caught between large ones.",
                                    Unread,
                                    vec![],
                                ),
                            ],
                        )
                    },
                    node(
                        WHAT_DRIVES_PLATES,
                        BroadPass,
                        "What drives the plates",
                        "Convection in the mantle and the pull of sinking slabs.",
                        Unread,
                        vec![],
                    ),
                ],
            },
            ChapterNode {
                id: CHAPTER_BOUNDARIES,
                title: "Where plates meet".to_owned(),
                sections: vec![
                    node(
                        KINDS_OF_BOUNDARY,
                        BroadPass,
                        "Kinds of boundary",
                        "Plates pull apart, collide or slide past each other.",
                        Unread,
                        vec![],
                    ),
                    node(
                        SUBDUCTION_ZONES,
                        BroadPass,
                        "Subduction zones",
                        "Where one plate sinks beneath another.",
                        Unread,
                        vec![],
                    ),
                ],
            },
        ],
        // Never opened: the first section.
        resume_at: Some(LAYERS),
        glossary_count: 2,
        side_chat_count: 1,
        placement: None,
    };
    assert_eq!(view, expected);
    assert_eq!(
        harness.summary(course).await.last_opened_at,
        Some(opened_at)
    );
    assert!(harness.provider.requests().is_empty());
}

#[tokio::test]
async fn a_course_in_placement_opens_at_its_placement_state() {
    let harness = Harness::new();
    let course = harness.store.create_course("Glaciers").unwrap().id;
    let opened_at = harness.advance();

    let view = harness.open(course).await;

    assert_eq!(view.stage, CourseStage::Placement);
    assert!(view.chapters.is_empty());
    assert_eq!(view.resume_at, None);
    assert_eq!(
        view.placement.map(|placement| placement.step),
        Some(PlacementStep::ChooseRating)
    );
    let summary = harness.summary(course).await;
    assert_eq!(summary.stage, CourseStage::Placement);
    assert_eq!(summary.last_opened_at, Some(opened_at));
    assert!(harness.provider.requests().is_empty());
}

#[tokio::test]
async fn opening_a_course_that_does_not_exist_is_not_found() {
    let harness = Harness::new();
    assert!(matches!(
        harness
            .send(CoreRequest::OpenCourse {
                course_id: Uuid::new_v4()
            })
            .await,
        Err(CoreError::NotFound { .. })
    ));
}

// -- deletion --------------------------------------------------------------------

#[tokio::test]
async fn deleting_a_course_tombstones_and_erases_everything_under_it() {
    let harness = Harness::new();
    let doomed = store_doomed_course(&harness.store);
    let kept = store_small_course(&harness.store, "Volcanoes", true);
    harness.advance();
    harness.open(kept.id).await;
    let kept_view = harness.open(kept.id).await;
    let kept_summary = harness.summary(kept.id).await;
    let raw = harness.raw();
    assert_eq!(
        sentinels_in_tables(&raw).len(),
        4,
        "every sentinel is stored before the deletion"
    );

    // The tables of syncable records are the ones checked below.
    let mut syncable = every_table(&raw);
    syncable.retain(|table| table != "local_settings");
    syncable.sort();
    let mut checked = course_tables();
    checked.sort();
    assert_eq!(syncable, checked);

    let before = dump(&raw, &course_tables());
    let owned = owned_rows(&raw, doomed);
    for table in TABLES {
        assert!(
            owned.iter().any(|(owner, _)| owner == table),
            "the course has rows in {table}"
        );
        assert!(
            before
                .keys()
                .any(|key| key.0 == table && !owned.contains(key)),
            "the kept course has rows in {table}"
        );
    }
    harness.events.take();
    let deleted_at = harness.advance();

    assert_eq!(
        harness
            .send(CoreRequest::DeleteCourse { course_id: doomed })
            .await,
        Ok(CoreResponse::Done)
    );

    // Not listed, not openable, the library told.
    let listed = harness.courses().await;
    assert_eq!(listed, vec![kept_summary.clone()]);
    assert_eq!(
        harness.events.take(),
        vec![CoreEvent::CoursesChanged {
            courses: vec![kept_summary.clone()]
        }]
    );
    assert!(matches!(
        harness
            .send(CoreRequest::OpenCourse { course_id: doomed })
            .await,
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        harness.next(doomed, LAYERS).await,
        Err(CoreError::NotFound { .. })
    ));
    assert!(matches!(
        harness
            .send(CoreRequest::DeleteCourse { course_id: doomed })
            .await,
        Err(CoreError::NotFound { .. })
    ));

    // Table by table: every row of the course kept as a tombstone with its
    // content erased; every other row as it was.
    let after = dump(&raw, &course_tables());
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>(),
        "no row is removed or added"
    );
    for (key, row) in &after {
        let old = &before[key];
        if !owned.contains(key) {
            assert_eq!(row, old, "{key:?} is untouched");
            continue;
        }
        let (table, _) = key;
        assert_eq!(row["id"], old["id"], "{key:?} keeps its id");
        assert_eq!(
            row["created_at"], old["created_at"],
            "{key:?} keeps its creation time"
        );
        assert_eq!(
            time(&row["deleted_at"]),
            deleted_at,
            "{key:?} is tombstoned"
        );
        assert_eq!(
            time(&row["updated_at"]),
            deleted_at,
            "{key:?}: the deletion is its last update"
        );
        for &column in content(table) {
            assert!(
                is_empty(&row[column]),
                "{key:?}.{column} is erased: {:?}",
                row[column]
            );
        }
    }

    // Nothing of it can be read back from the tables or the file.
    assert_eq!(sentinels_in_tables(&raw), BTreeSet::new());
    drop(raw);
    for entry in std::fs::read_dir(harness.path.parent().unwrap()).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        for sentinel in [
            SECTION_SENTINEL,
            RECORD_SENTINEL,
            CONVENTION_SENTINEL,
            MESSAGE_SENTINEL,
        ] {
            assert!(
                !bytes
                    .windows(sentinel.len())
                    .any(|window| window == sentinel.as_bytes()),
                "{sentinel} is still in {}",
                path.display()
            );
        }
    }

    // The other course is untouched.
    assert_eq!(harness.open(kept.id).await, kept_view);
    assert!(harness.provider.requests().is_empty());
}

#[tokio::test]
async fn a_deletion_that_fails_part_way_deletes_nothing() {
    let harness = Harness::new();
    let doomed = store_doomed_course(&harness.store);
    store_small_course(&harness.store, "Volcanoes", true);
    let raw = harness.raw();
    let before = dump(&raw, &course_tables());
    let listed = harness.courses().await;
    harness.events.take();
    harness.advance();

    // Whichever table the deletion reaches, a refusal there undoes the rest.
    for table in TABLES {
        raw.execute_batch(&format!(
            "CREATE TRIGGER refuse_the_deletion BEFORE UPDATE ON {table}
             BEGIN SELECT RAISE(ABORT, 'refused'); END;"
        ))
        .unwrap();

        let result = harness
            .send(CoreRequest::DeleteCourse { course_id: doomed })
            .await;

        raw.execute_batch("DROP TRIGGER refuse_the_deletion")
            .unwrap();
        assert!(
            matches!(result, Err(CoreError::Store { .. })),
            "refused at {table}: {result:?}"
        );
        assert!(
            dump(&raw, &course_tables()) == before,
            "refused at {table}: nothing is deleted"
        );
    }

    assert_eq!(harness.courses().await, listed);
    assert_eq!(harness.events.take(), vec![]);
    assert_eq!(harness.open(doomed).await.id, doomed);
}

#[tokio::test]
async fn deleting_a_course_erases_a_version_tombstoned_before() {
    let harness = Harness::new();
    let doomed = store_doomed_course(&harness.store);
    let tombstoned_at = harness.advance();
    assert!(
        harness
            .store
            .delete(RecordKind::SectionVersion, LAYERS_V1)
            .unwrap()
    );
    let raw = harness.raw();
    assert!(
        sentinels_in_tables(&raw).is_superset(&BTreeSet::from([SECTION_SENTINEL, RECORD_SENTINEL])),
        "a tombstoned version still holds its text and record"
    );
    let deleted_at = harness.advance();

    assert_eq!(
        harness
            .send(CoreRequest::DeleteCourse { course_id: doomed })
            .await,
        Ok(CoreResponse::Done)
    );

    let after = dump(&raw, &course_tables());
    let earlier = ("section_versions".to_owned(), LAYERS_V1.to_string());
    let owned = owned_rows(&raw, doomed);
    assert!(owned.contains(&earlier));
    for key in &owned {
        let row = &after[key];
        let tombstone = if *key == earlier {
            tombstoned_at
        } else {
            deleted_at
        };
        assert_eq!(
            time(&row["deleted_at"]),
            tombstone,
            "{key:?} keeps its first deletion time"
        );
        assert_eq!(
            time(&row["updated_at"]),
            deleted_at,
            "{key:?}: the deletion is its last update"
        );
        for &column in content(&key.0) {
            assert!(
                is_empty(&row[column]),
                "{key:?}.{column} is erased: {:?}",
                row[column]
            );
        }
    }
    assert_eq!(sentinels_in_tables(&raw), BTreeSet::new());
}

/// A string only a section generated after its course was deleted holds.
const LATE_SENTINEL: &str = "sentinel-in-a-late-section-5d80";

/// A provider that holds each stream until released, and tells when it holds
/// one.
struct HeldProvider {
    inner: Arc<FakeProvider>,
    held: Notify,
    release: Notify,
}

#[async_trait]
impl ModelProvider for HeldProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        self.held.notify_one();
        self.release.notified().await;
        self.inner.stream(request).await
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        self.inner.list_models().await
    }
}

impl Harness {
    /// A harness whose core asks `provider`; `self.provider` is its fake.
    fn holding(provider: Arc<HeldProvider>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        Self {
            _dir: dir,
            path,
            ports,
            provider: provider.inner.clone(),
            events,
            core,
            store,
        }
    }
}

#[tokio::test]
async fn a_section_written_after_its_course_is_deleted_is_not_stored() {
    let provider = Arc::new(HeldProvider {
        inner: Arc::new(FakeProvider::new(FakeScript {
            section_deltas: vec![format!("Written too late. {LATE_SENTINEL}")],
            ..FakeScript::sample()
        })),
        held: Notify::new(),
        release: Notify::new(),
    });
    let harness = Harness::holding(provider.clone());
    let doomed = store_small_course(&harness.store, "Volcanoes", true);
    let unwritten = doomed.sections[1];

    // The section is being written when the course is deleted.
    let (opened, ()) = tokio::join!(
        harness.send(CoreRequest::OpenSection {
            course_id: doomed.id,
            section_id: unwritten,
        }),
        async {
            provider.held.notified().await;
            assert_eq!(
                harness
                    .send(CoreRequest::DeleteCourse {
                        course_id: doomed.id
                    })
                    .await,
                Ok(CoreResponse::Done)
            );
            provider.release.notify_one();
        }
    );

    assert!(
        matches!(opened, Err(CoreError::NotFound { .. })),
        "{opened:?}"
    );
    assert_eq!(
        harness.provider.requests().len(),
        1,
        "the section was written"
    );
    let raw = harness.raw();
    let rows = dump(&raw, &course_tables());
    for key in owned_rows(&raw, doomed.id) {
        let row = &rows[&key];
        assert!(!is_empty(&row["deleted_at"]), "{key:?} is tombstoned");
        for &column in content(&key.0) {
            assert!(
                is_empty(&row[column]),
                "{key:?}.{column} is erased: {:?}",
                row[column]
            );
        }
    }
    let everything = dump(&raw, &every_table(&raw));
    assert!(
        !everything
            .values()
            .flat_map(BTreeMap::values)
            .any(|value| matches!(value, Value::Text(text) if text.contains(LATE_SENTINEL))),
        "the late section is stored"
    );
}

// -- Next ------------------------------------------------------------------------

#[tokio::test]
async fn next_answers_the_section_the_index_leads_to() {
    let harness = Harness::new();
    let course = store_course(&harness.store, &sample_course(), None);
    harness.events.take();
    let leads = |section_id| -> Result<CoreResponse, CoreError> {
        Ok(CoreResponse::NextSection {
            course_id: course,
            section_id,
        })
    };

    // From a broad-pass section: the following one, across chapters, past
    // the deep dive under it.
    assert_eq!(harness.next(course, LAYERS).await, leads(LITHOSPHERE));
    assert_eq!(
        harness.next(course, LITHOSPHERE).await,
        leads(WHAT_A_PLATE_IS)
    );
    assert_eq!(
        harness.next(course, WHAT_A_PLATE_IS).await,
        leads(WHAT_DRIVES_PLATES)
    );
    // From a deep dive's middle sibling: the next sibling.
    assert_eq!(
        harness.next(course, PLATE_THICKNESS).await,
        leads(MICROPLATES)
    );
    // From its last sibling: the broad-pass section after its root.
    assert_eq!(
        harness.next(course, MICROPLATES).await,
        leads(WHAT_DRIVES_PLATES)
    );
    // From the last section of the course: the end.
    assert_eq!(
        harness.next(course, SUBDUCTION_ZONES).await,
        Ok(CoreResponse::EndOfCourse { course_id: course })
    );

    // It only answers: nothing generated, opened, marked read or moved.
    assert!(harness.provider.requests().is_empty());
    assert_eq!(harness.events.take(), vec![]);
    let summary = harness.summary(course).await;
    assert_eq!(summary.progress, Progress { read: 4, total: 9 });
    assert_eq!(summary.resume_title.as_deref(), Some("Layers of the Earth"));
    assert_eq!(summary.last_opened_at, None);
}

#[tokio::test]
async fn next_from_a_section_not_in_the_course_is_not_found() {
    let harness = Harness::new();
    let course = store_course(&harness.store, &sample_course(), None);
    let other = store_small_course(&harness.store, "Volcanoes", false);

    for section in [Uuid::new_v4(), other.sections[0]] {
        assert!(matches!(
            harness.next(course, section).await,
            Err(CoreError::NotFound { .. })
        ));
    }
    assert!(matches!(
        harness.next(Uuid::new_v4(), LAYERS).await,
        Err(CoreError::NotFound { .. })
    ));
}
