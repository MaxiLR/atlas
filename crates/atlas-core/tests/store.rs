//! The store through its repository API, on a database file in a temp dir
//! with a fake clock. Raw SQL is used only to look under the API: the schema
//! version, what the database refuses, and the rows a tombstone leaves.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::{Concept, MapEntry};
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::profile::{Knowledge, Misconception};
use atlas_core::domain::questions::{KeyedCheckQuestion, KeyedOption, KeyedPlacementQuestion};
use atlas_core::domain::record::{RecordStatus, SectionRecord, StoredRecord, TaughtConcept};
use atlas_core::index_tree::TreeSection;
use atlas_core::ports::Clock;
use atlas_core::protocol::{
    Anchor, Block, BlockContent, ChatRole, CourseStage, Goal, MessageState, PlacementAnswer,
    SelfRating, Source, VersionStyle, WikipediaResult,
};
use atlas_core::store::{
    CheckBlock, ConventionOutcome, NewChatMessage, NewGlossaryEntry, NewSectionVersion, RecordKind,
    SCHEMA_VERSION, Store, StoreError,
};
use atlas_core::testing::FakeClock;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension};
use support::sample_course::{
    CHAPTER_MOTION, KINDS_OF_BOUNDARY, LAYERS, LAYERS_V1, LITHOSPHERE, LITHOSPHERE_V1, MICROPLATES,
    SampleCourse, WHAT_A_PLATE_IS, WHAT_A_PLATE_IS_V1, WHAT_DRIVES_PLATES, sample_course,
};
use tempfile::TempDir;
use uuid::Uuid;

/// 2026-03-04T05:06:07Z, the fake clock's start.
fn start() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).single().unwrap()
}

struct Fixture {
    _dir: TempDir,
    path: PathBuf,
    clock: Arc<FakeClock>,
    store: Store,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let clock = Arc::new(FakeClock::new(start()));
        let store = Store::open(&path, clock.clone()).unwrap();
        Self {
            _dir: dir,
            path,
            clock,
            store,
        }
    }

    fn raw(&self) -> Connection {
        Connection::open(&self.path).unwrap()
    }

    /// Moves the clock on by an hour and returns the new time.
    fn advance(&self) -> DateTime<Utc> {
        self.clock.advance(Duration::from_secs(3600));
        self.clock.now()
    }
}

fn prose(text: &str) -> Vec<Block> {
    vec![Block {
        index: 0,
        content: BlockContent::Prose {
            text: text.to_owned(),
        },
    }]
}

fn new_version(id: Uuid, node: Uuid, record: StoredRecord) -> NewSectionVersion {
    NewSectionVersion {
        id,
        node,
        style: VersionStyle::Original,
        blocks: prose(&format!("The text of version {id}.")),
        checks: Vec::new(),
        citations: Vec::new(),
        sources: Vec::new(),
        written_without_search: false,
        record,
        model: Some("gpt-5".to_owned()),
        prompt: "write-section@1".to_owned(),
    }
}

fn simple_record(concept: &str) -> StoredRecord {
    StoredRecord {
        record: SectionRecord {
            summary: Some(format!("A section about {concept}.")),
            concepts: vec![TaughtConcept {
                name: concept.to_owned(),
                line: Some(format!("What {concept} is.")),
            }],
            notation: Vec::new(),
            terms: Vec::new(),
            examples: Vec::new(),
            promises: Vec::new(),
        },
        status: RecordStatus::Confirmed,
    }
}

/// Stores the sample course: ladder, profile, index with its map and the
/// conventions fixed by the plan, then each read section's version with its
/// record and the conventions that version fixed.
fn store_sample(store: &Store) -> (Uuid, SampleCourse) {
    let sample = sample_course();
    let course = store.create_course(&sample.topic).unwrap().id;
    store.store_ladder(course, &sample.ladder).unwrap();
    store.save_profile(course, &sample.profile).unwrap();
    let plan: Vec<Convention> = sample
        .conventions
        .iter()
        .filter(|convention| convention.origin == ConventionOrigin::Plan)
        .cloned()
        .collect();
    let outcomes = store
        .insert_index(course, &sample.index, &sample.map, &plan)
        .unwrap();
    assert_eq!(outcomes, vec![ConventionOutcome::Stored; plan.len()]);
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
        let outcomes = store
            .insert_version(
                &new_version(record.version, record.section, record.record.clone()),
                &fixed,
            )
            .unwrap();
        assert_eq!(outcomes, vec![ConventionOutcome::Stored; fixed.len()]);
    }
    (course, sample)
}

fn user_version(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn parse_time(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap_or_else(|error| panic!("timestamp {text:?}: {error}"))
        .with_timezone(&Utc)
}

/// The id, creation, update and deletion times of a row, as stored.
type Stamps = (String, DateTime<Utc>, DateTime<Utc>, Option<DateTime<Utc>>);

fn stamps(connection: &Connection, table: &str, id: Uuid) -> Option<Stamps> {
    connection
        .query_row(
            &format!("SELECT id, created_at, updated_at, deleted_at FROM {table} WHERE id = ?1"),
            [id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .unwrap()
        .map(|(id, created, updated, deleted)| {
            (
                id,
                parse_time(&created),
                parse_time(&updated),
                deleted.as_deref().map(parse_time),
            )
        })
}

/// The tables that hold syncable records: every table but SQLite's own and
/// the local-only settings.
fn syncable_tables(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table'
             AND name NOT LIKE 'sqlite_%' AND name <> 'local_settings' ORDER BY name",
        )
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn any_id(connection: &Connection, table: &str) -> Uuid {
    let id: String = connection
        .query_row(
            &format!("SELECT id FROM {table} WHERE deleted_at IS NULL ORDER BY rowid LIMIT 1"),
            [],
            |row| row.get(0),
        )
        .unwrap();
    Uuid::parse_str(&id).unwrap()
}

fn anchor(version: Uuid, quote: &str) -> Anchor {
    Anchor {
        version_id: version,
        block: 0,
        quote: quote.to_owned(),
    }
}

// Migrations.

// The schema version is a constant; asserting it documents that 0001 is not
// the latest.
#[allow(clippy::assertions_on_constants)]
#[test]
fn a_fresh_database_migrates_to_the_latest_version() {
    let fixture = Fixture::new();
    assert!(SCHEMA_VERSION > 1, "the full schema adds to migration 0001");
    assert_eq!(user_version(&fixture.path), SCHEMA_VERSION);
    drop(fixture.store);
    // Opening again applies nothing twice.
    Store::open(&fixture.path, fixture.clock.clone()).unwrap();
    assert_eq!(user_version(&fixture.path), SCHEMA_VERSION);
}

#[test]
fn a_version_1_database_migrates_with_its_rows_intact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("atlas.sqlite3");
    let course = Uuid::new_v4();
    let chapter = Uuid::new_v4();
    let section = Uuid::new_v4();
    let version = Uuid::new_v4();
    let written = "2025-12-01T10:00:00.000000Z";
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!("../src/store/migrations/0001_thin_loop.sql"))
            .unwrap();
        connection
            .pragma_update(None, "user_version", 1_i64)
            .unwrap();
        connection
            .execute(
                "INSERT INTO courses (id, topic, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                rusqlite::params![course.to_string(), "Volcanoes", written],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO index_nodes
                    (id, course_id, parent_id, kind, position, title, summary, created_at, updated_at)
                 VALUES (?1, ?2, NULL, 'chapter', 0, 'Eruptions', '', ?3, ?3),
                        (?4, ?2, ?1, 'section', 0, 'Magma', 'Where magma comes from.', ?3, ?3)",
                rusqlite::params![
                    chapter.to_string(),
                    course.to_string(),
                    written,
                    section.to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO section_versions (id, node_id, content, prompt, created_at, updated_at)
                 VALUES (?1, ?2, 'Magma rises from the mantle.', 'write-section@1', ?3, ?3)",
                rusqlite::params![version.to_string(), section.to_string(), written],
            )
            .unwrap();
    }

    let store = Store::open(&path, Arc::new(FakeClock::new(start()))).unwrap();
    assert_eq!(user_version(&path), SCHEMA_VERSION);

    let stored = store.course_record(course).unwrap().expect("the course");
    assert_eq!(stored.topic, "Volcanoes");
    assert_eq!(stored.stage, CourseStage::Ready);
    assert_eq!(stored.created_at, parse_time(written));
    assert_eq!(stored.updated_at, parse_time(written));

    let tree = store.index_tree(course).unwrap();
    assert_eq!(tree.chapters.len(), 1);
    assert_eq!(tree.chapters[0].id, chapter);
    assert_eq!(tree.chapters[0].title, "Eruptions");
    assert_eq!(tree.chapters[0].sections.len(), 1);
    assert_eq!(tree.chapters[0].sections[0].id, section);
    assert_eq!(tree.chapters[0].sections[0].title, "Magma");
    assert_eq!(tree.chapters[0].sections[0].line, "Where magma comes from.");

    let current = store
        .current_version(section)
        .unwrap()
        .expect("the stored version is current");
    assert_eq!(current.id, version);
    assert_eq!(current.blocks, prose("Magma rises from the mantle."));
    assert_eq!(current.prompt, "write-section@1");
    assert_eq!(current.created_at, parse_time(written));
    assert_eq!(current.record, None);
}

#[test]
fn a_database_newer_than_the_code_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("atlas.sqlite3");
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();
    let error = match Store::open(&path, Arc::new(FakeClock::new(start()))) {
        Ok(_) => panic!("a newer database must be refused"),
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            StoreError::TooNew { found, supported }
                if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
        ),
        "{error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("newer"), "{message}");
    assert!(
        message.contains(&(SCHEMA_VERSION + 1).to_string()),
        "{message}"
    );
    // Nothing was migrated.
    assert_eq!(user_version(&path), SCHEMA_VERSION + 1);
}

// Section versions.

#[test]
fn a_stored_version_and_its_record_cannot_be_changed_but_the_pointer_moves() {
    let fixture = Fixture::new();
    let (_, sample) = store_sample(&fixture.store);
    let before = fixture.store.version(LAYERS_V1).unwrap().expect("stored");
    assert_eq!(before.record.as_ref(), Some(&sample.records[0].record));

    let raw = fixture.raw();
    for change in [
        "UPDATE section_versions SET blocks = '[]' WHERE id = ?1",
        "UPDATE section_versions SET record = NULL, record_status = NULL WHERE id = ?1",
        "UPDATE section_versions SET record_status = 'predicted' WHERE id = ?1",
        "UPDATE section_versions SET prompt = 'other@2' WHERE id = ?1",
        "UPDATE section_versions SET updated_at = '2030-01-01T00:00:00.000000Z' WHERE id = ?1",
    ] {
        assert!(
            raw.execute(change, [LAYERS_V1.to_string()]).is_err(),
            "the database allowed: {change}"
        );
    }
    assert_eq!(fixture.store.version(LAYERS_V1).unwrap(), Some(before));

    let second = Uuid::new_v4();
    fixture
        .store
        .insert_version(&new_version(second, LAYERS, simple_record("crust")), &[])
        .unwrap();
    assert_eq!(
        fixture.store.current_version(LAYERS).unwrap().unwrap().id,
        second
    );
    fixture
        .store
        .set_current_version(LAYERS, LAYERS_V1)
        .unwrap();
    assert_eq!(
        fixture.store.current_version(LAYERS).unwrap().unwrap().id,
        LAYERS_V1
    );
    let versions: Vec<Uuid> = fixture
        .store
        .versions(LAYERS)
        .unwrap()
        .into_iter()
        .map(|version| version.id)
        .collect();
    assert_eq!(versions, vec![LAYERS_V1, second]);
}

#[test]
fn a_stored_version_cannot_be_replaced_or_deleted() {
    let fixture = Fixture::new();
    store_sample(&fixture.store);
    let second = Uuid::new_v4();
    fixture
        .store
        .insert_version(&new_version(second, LAYERS, simple_record("crust")), &[])
        .unwrap();
    let first_before = fixture.store.version(LAYERS_V1).unwrap().expect("stored");
    let second_before = fixture.store.version(second).unwrap().expect("stored");

    let raw = fixture.raw();
    let columns = "id, node_id, style, blocks, checks, citations, sources, \
                   written_without_search, record, record_status, model, prompt, \
                   created_at, updated_at";
    let changed = "node_id, style, '[]', checks, citations, sources, \
                   written_without_search, NULL, NULL, model, prompt, created_at, updated_at";
    for change in [
        // A replace that conflicts on the id.
        format!(
            "INSERT OR REPLACE INTO section_versions ({columns})
             SELECT id, {changed} FROM section_versions WHERE id = ?1"
        ),
        format!(
            "REPLACE INTO section_versions ({columns})
             SELECT id, {changed} FROM section_versions WHERE id = ?1"
        ),
        // A replace that conflicts on the row id under a new id.
        format!(
            "INSERT OR REPLACE INTO section_versions (rowid, {columns})
             SELECT rowid, '{}', {changed} FROM section_versions WHERE id = ?1",
            Uuid::new_v4()
        ),
        // An upsert.
        format!(
            "INSERT INTO section_versions ({columns})
             SELECT id, {changed} FROM section_versions WHERE id = ?1
             ON CONFLICT (id) DO UPDATE SET blocks = excluded.blocks"
        ),
        // A tombstone that moves onto another version's row id.
        format!(
            "UPDATE OR REPLACE section_versions
             SET rowid = (SELECT rowid FROM section_versions WHERE id = '{second}'),
                 deleted_at = '2030-01-01T00:00:00.000000Z'
             WHERE id = ?1"
        ),
        "DELETE FROM section_versions WHERE id = ?1".to_owned(),
    ] {
        assert!(
            raw.execute(&change, [LAYERS_V1.to_string()]).is_err(),
            "the database allowed: {change}"
        );
    }
    assert_eq!(
        fixture.store.version(LAYERS_V1).unwrap(),
        Some(first_before)
    );
    assert_eq!(fixture.store.version(second).unwrap(), Some(second_before));
}

#[test]
fn a_version_of_another_node_cannot_become_current() {
    let fixture = Fixture::new();
    store_sample(&fixture.store);
    assert!(
        fixture
            .store
            .set_current_version(LAYERS, LITHOSPHERE_V1)
            .is_err()
    );
    assert_eq!(
        fixture.store.current_version(LAYERS).unwrap().unwrap().id,
        LAYERS_V1
    );
}

#[test]
fn the_current_record_is_the_current_versions() {
    let fixture = Fixture::new();
    let (_, sample) = store_sample(&fixture.store);
    let first = sample.records[0].record.clone();
    let second_record = simple_record("the inner core");
    let second = Uuid::new_v4();
    fixture
        .store
        .insert_version(&new_version(second, LAYERS, second_record.clone()), &[])
        .unwrap();
    assert_eq!(
        fixture.store.current_record(LAYERS).unwrap(),
        Some(second_record.clone())
    );
    fixture
        .store
        .set_current_version(LAYERS, LAYERS_V1)
        .unwrap();
    assert_eq!(fixture.store.current_record(LAYERS).unwrap(), Some(first));
    fixture.store.set_current_version(LAYERS, second).unwrap();
    assert_eq!(
        fixture.store.current_record(LAYERS).unwrap(),
        Some(second_record)
    );
}

#[test]
fn a_version_keeps_its_blocks_checks_sources_and_metadata() {
    let fixture = Fixture::new();
    store_sample(&fixture.store);
    let check = CheckBlock {
        block: 1,
        questions: vec![KeyedCheckQuestion {
            rung: RungId(2),
            stem: "What rides on the asthenosphere?".to_owned(),
            options: ["The core", "The lithosphere", "The mantle", "The ocean"]
                .iter()
                .enumerate()
                .map(|(index, text)| KeyedOption {
                    text: (*text).to_owned(),
                    misconception: (index != 1).then(|| format!("Thinks {text} floats.")),
                })
                .collect(),
            key: 1,
            explanation: "The rigid shell rides on the soft layer.".to_owned(),
        }],
    };
    let mut version = new_version(
        Uuid::new_v4(),
        LITHOSPHERE,
        StoredRecord {
            status: RecordStatus::Predicted,
            ..simple_record("lithosphere")
        },
    );
    version.style = VersionStyle::MoreExamples;
    version.checks = vec![check];
    version.sources = vec![Source {
        number: 1,
        url: "https://example.org/plates".to_owned(),
        title: "Plates".to_owned(),
    }];
    version.written_without_search = true;
    fixture.store.insert_version(&version, &[]).unwrap();

    let stored = fixture.store.version(version.id).unwrap().unwrap();
    assert_eq!(stored.node_id, LITHOSPHERE);
    assert_eq!(stored.style, VersionStyle::MoreExamples);
    assert_eq!(stored.blocks, version.blocks);
    assert_eq!(stored.checks, version.checks);
    assert_eq!(stored.citations, version.citations);
    assert_eq!(stored.sources, version.sources);
    assert!(stored.written_without_search);
    assert_eq!(stored.record, Some(version.record.clone()));
    assert_eq!(stored.model.as_deref(), Some("gpt-5"));
    assert_eq!(stored.prompt, "write-section@1");
}

// Course memory.

#[test]
fn the_sample_course_map_and_conventions_are_read_back_equal() {
    let fixture = Fixture::new();
    let (course, sample) = store_sample(&fixture.store);
    assert_eq!(fixture.store.course_map(course).unwrap(), sample.map);
    assert_eq!(
        fixture.store.conventions(course).unwrap(),
        sample.conventions
    );
    assert_eq!(fixture.store.index_tree(course).unwrap(), sample.index);
    assert_eq!(
        fixture.store.map_entry(WHAT_A_PLATE_IS).unwrap().as_ref(),
        sample
            .map
            .entries
            .iter()
            .find(|entry| entry.section == WHAT_A_PLATE_IS)
    );
    assert_eq!(fixture.store.ladder(course).unwrap(), Some(sample.ladder));
    assert_eq!(fixture.store.profile(course).unwrap(), Some(sample.profile));
}

#[test]
fn children_and_their_map_entries_are_added_without_touching_existing_ones() {
    let fixture = Fixture::new();
    let (course, sample) = store_sample(&fixture.store);
    let raw = fixture.raw();
    let before: Vec<(String, Vec<Stamps>)> = syncable_tables(&raw)
        .into_iter()
        .map(|table| {
            let ids: Vec<String> = raw
                .prepare(&format!("SELECT id FROM {table} ORDER BY rowid"))
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let rows = ids
                .iter()
                .map(|id| stamps(&raw, &table, Uuid::parse_str(id).unwrap()).unwrap())
                .collect();
            (table, rows)
        })
        .collect();

    let later = fixture.advance();
    let ridge = Uuid::new_v4();
    let trench = Uuid::new_v4();
    let child = |id, title: &str| TreeSection {
        id,
        title: title.to_owned(),
        line: format!("About {title}."),
        read: false,
        children: Vec::new(),
    };
    let children = vec![child(ridge, "Mid-ocean ridges"), child(trench, "Trenches")];
    let concepts = vec![
        Concept {
            name: "mid-ocean ridge".to_owned(),
            rung: Some(RungId(6)),
        },
        Concept {
            name: "trench".to_owned(),
            rung: None,
        },
    ];
    let entries = vec![
        MapEntry {
            section: ridge,
            introduces: vec!["mid-ocean ridge".to_owned()],
            relies_on: vec!["divergent boundary".to_owned()],
        },
        MapEntry {
            section: trench,
            introduces: vec!["trench".to_owned()],
            relies_on: vec![
                "convergent boundary".to_owned(),
                "mid-ocean ridge".to_owned(),
            ],
        },
    ];
    fixture
        .store
        .insert_children(course, KINDS_OF_BOUNDARY, &children, &concepts, &entries)
        .unwrap();

    let mut expected_map = sample.map.clone();
    expected_map.concepts.extend(concepts);
    expected_map.entries.extend(entries);
    assert_eq!(fixture.store.course_map(course).unwrap(), expected_map);

    let tree = fixture.store.index_tree(course).unwrap();
    let kinds = &tree.chapters[2].sections[0];
    assert_eq!(kinds.id, KINDS_OF_BOUNDARY);
    assert_eq!(kinds.children, children);

    // Every row stored before is exactly as it was.
    for (table, rows) in before {
        for row in rows {
            let id = Uuid::parse_str(&row.0).unwrap();
            assert_eq!(stamps(&raw, &table, id), Some(row), "{table} {id}");
        }
    }
    let ridge_row = stamps(&raw, "index_nodes", ridge).unwrap();
    assert_eq!((ridge_row.1, ridge_row.2), (later, later));
}

#[test]
fn a_children_write_with_an_unknown_concept_stores_nothing() {
    let fixture = Fixture::new();
    let (course, sample) = store_sample(&fixture.store);
    let child = Uuid::new_v4();
    let result = fixture.store.insert_children(
        course,
        MICROPLATES,
        &[TreeSection {
            id: child,
            title: "Plate fragments".to_owned(),
            line: "Pieces broken off larger plates.".to_owned(),
            read: false,
            children: Vec::new(),
        }],
        &[],
        &[MapEntry {
            section: child,
            introduces: vec!["no such concept".to_owned()],
            relies_on: Vec::new(),
        }],
    );
    assert!(result.is_err());
    assert_eq!(fixture.store.index_tree(course).unwrap(), sample.index);
    assert_eq!(fixture.store.course_map(course).unwrap(), sample.map);
}

#[test]
fn a_convention_whose_normalised_key_exists_is_rejected() {
    let fixture = Fixture::new();
    let (course, sample) = store_sample(&fixture.store);
    let clash = Convention {
        kind: ConventionKind::Terminology,
        key: "  PLATE\t".to_owned(),
        meaning: "a dinner plate".to_owned(),
        origin: ConventionOrigin::SectionVersion {
            version: WHAT_A_PLATE_IS_V1,
        },
    };
    assert_eq!(
        fixture.store.add_convention(course, &clash).unwrap(),
        ConventionOutcome::Rejected
    );
    assert_eq!(
        fixture.store.conventions(course).unwrap(),
        sample.conventions
    );

    // Inside a version's transaction the clash is rejected too, while the
    // version and the other conventions are stored.
    let version = Uuid::new_v4();
    let new_term = Convention {
        kind: ConventionKind::Terminology,
        key: "slab".to_owned(),
        meaning: "the sinking part of a plate".to_owned(),
        origin: ConventionOrigin::SectionVersion { version },
    };
    let outcomes = fixture
        .store
        .insert_version(
            &new_version(version, WHAT_A_PLATE_IS, simple_record("tectonic plate")),
            &[
                Convention {
                    origin: ConventionOrigin::SectionVersion { version },
                    ..clash.clone()
                },
                new_term.clone(),
            ],
        )
        .unwrap();
    assert_eq!(
        outcomes,
        vec![ConventionOutcome::Rejected, ConventionOutcome::Stored]
    );
    let mut expected = sample.conventions.clone();
    expected.push(new_term);
    assert_eq!(fixture.store.conventions(course).unwrap(), expected);

    // The same key under another kind, or in another course, is its own.
    let notation = Convention {
        kind: ConventionKind::Notation,
        key: "plate".to_owned(),
        meaning: "a symbol for a plate".to_owned(),
        origin: ConventionOrigin::Plan,
    };
    assert_eq!(
        fixture.store.add_convention(course, &notation).unwrap(),
        ConventionOutcome::Stored
    );
    let other = fixture.store.create_course("Glaciers").unwrap().id;
    let plan_term = Convention {
        origin: ConventionOrigin::Plan,
        ..clash
    };
    assert_eq!(
        fixture.store.add_convention(other, &plan_term).unwrap(),
        ConventionOutcome::Stored
    );
    assert_eq!(fixture.store.conventions(other).unwrap(), vec![plan_term]);
}

#[test]
fn the_database_normalises_a_conventions_key_and_refuses_a_second_live_one() {
    let fixture = Fixture::new();
    let (course, _) = store_sample(&fixture.store);
    let raw = fixture.raw();

    // A writer cannot supply its own normalised key.
    assert!(
        raw.execute(
            "INSERT INTO conventions
                (id, course_id, kind, key, normalised_key, meaning, created_at, updated_at)
             VALUES (?1, ?2, 'terminology', 'PLATE', 'PLATE', 'another meaning',
                     '2026-03-04T05:06:07.000000Z', '2026-03-04T05:06:07.000000Z')",
            rusqlite::params![Uuid::new_v4().to_string(), course.to_string()],
        )
        .is_err()
    );

    let insert = |key: &str| {
        raw.execute(
            "INSERT INTO conventions (id, course_id, kind, key, meaning, created_at, updated_at)
             VALUES (?1, ?2, 'terminology', ?3, 'another meaning',
                     '2026-03-04T05:06:07.000000Z', '2026-03-04T05:06:07.000000Z')",
            rusqlite::params![Uuid::new_v4().to_string(), course.to_string(), key],
        )
    };
    for clash in ["PLATE", "  Plate\t", "\u{3000}plate\u{a0}"] {
        assert!(insert(clash).is_err(), "the database stored {clash:?}");
    }
    assert!(insert("Slab").is_ok());
    assert!(insert(" SLAB\n").is_err());

    // Once the first is deleted the key is free again.
    let first = any_id(&raw, "conventions");
    assert!(fixture.store.delete(RecordKind::Convention, first).unwrap());
    assert!(insert("PLATE ").is_ok());
    let normalised: String = raw
        .query_row(
            "SELECT normalised_key FROM conventions WHERE key = 'PLATE '",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(normalised, "plate");
}

// Courses, placement, profile, checks.

#[test]
fn a_course_moves_through_its_stages_with_its_placement_kept() {
    let fixture = Fixture::new();
    let created = fixture.store.create_course("Tides").unwrap();
    assert_eq!(created.topic, "Tides");
    assert_eq!(created.stage, CourseStage::Placement);
    assert_eq!(created.goal, None);
    assert_eq!(created.last_opened_at, None);
    assert_eq!(created.last_read, None);
    assert_eq!((created.created_at, created.updated_at), (start(), start()));
    let course = created.id;

    fixture
        .store
        .set_self_rating(course, SelfRating::KnowTheBasics, Goal::Overview)
        .unwrap();
    let question = |rung: u32| KeyedPlacementQuestion {
        id: Uuid::new_v4(),
        rung: RungId(rung),
        stem: format!("Question on rung {rung}"),
        options: (0..4)
            .map(|option| KeyedOption {
                text: format!("Option {option}"),
                misconception: (option != 0).then(|| format!("Mistake {option}")),
            })
            .collect(),
        key: 0,
    };
    let first = question(3);
    let second = question(5);
    fixture
        .store
        .add_placement_question(course, &first)
        .unwrap();
    fixture
        .store
        .answer_placement_question(first.id, PlacementAnswer::Option { index: 2 })
        .unwrap();
    fixture
        .store
        .add_placement_question(course, &second)
        .unwrap();

    let asked = fixture.store.placement_questions(course).unwrap();
    assert_eq!(asked.len(), 2);
    assert_eq!(
        (asked[0].number, &asked[0].question, asked[0].answer),
        (1, &first, Some(PlacementAnswer::Option { index: 2 }))
    );
    assert_eq!(
        (asked[1].number, &asked[1].question, asked[1].answer),
        (2, &second, None)
    );

    let record = fixture.store.course_record(course).unwrap().unwrap();
    assert_eq!(record.self_rating, Some(SelfRating::KnowTheBasics));
    assert_eq!(record.goal, Some(Goal::Overview));

    fixture.store.set_stage(course, CourseStage::Ready).unwrap();
    fixture.store.mark_opened(course).unwrap();
    let record = fixture.store.course_record(course).unwrap().unwrap();
    assert_eq!(record.stage, CourseStage::Ready);
    assert_eq!(record.last_opened_at, Some(start()));
}

#[test]
fn the_library_lists_courses_most_recently_opened_first() {
    let fixture = Fixture::new();
    let first = fixture.store.create_course("First").unwrap().id;
    fixture.advance();
    let second = fixture.store.create_course("Second").unwrap().id;
    fixture.advance();
    let third = fixture.store.create_course("Third").unwrap().id;
    fixture.advance();
    fixture.store.mark_opened(first).unwrap();
    let order: Vec<Uuid> = fixture
        .store
        .course_records()
        .unwrap()
        .into_iter()
        .map(|course| course.id)
        .collect();
    assert_eq!(order, vec![first, third, second]);
}

#[test]
fn the_profile_and_check_answers_are_kept() {
    let fixture = Fixture::new();
    let (course, sample) = store_sample(&fixture.store);
    let mut profile = sample.profile.clone();
    profile.rungs[2].state = Knowledge::Known;
    profile.misconceptions.push(Misconception {
        rung: RungId(5),
        text: "Plates only collide.".to_owned(),
    });
    fixture.store.save_profile(course, &profile).unwrap();
    assert_eq!(fixture.store.profile(course).unwrap(), Some(profile));

    let answer = fixture
        .store
        .add_check_answer(LAYERS_V1, 1, 0, 2, false)
        .unwrap();
    assert_eq!(
        (
            answer.version_id,
            answer.block,
            answer.question,
            answer.choice,
            answer.correct
        ),
        (LAYERS_V1, 1, 0, 2, false)
    );
    assert_eq!(
        fixture.store.check_answer(LAYERS_V1, 1, 0).unwrap(),
        Some(answer.clone())
    );
    assert_eq!(fixture.store.check_answer(LAYERS_V1, 1, 1).unwrap(), None);
    assert_eq!(
        fixture.store.check_answers(LAYERS_V1).unwrap(),
        vec![answer]
    );
}

#[test]
fn the_index_records_reading_and_the_last_read_section() {
    let fixture = Fixture::new();
    let (course, _) = store_sample(&fixture.store);
    let later = fixture.advance();
    fixture.store.mark_read(WHAT_DRIVES_PLATES).unwrap();
    fixture
        .store
        .set_last_read(course, WHAT_DRIVES_PLATES)
        .unwrap();
    let node = fixture
        .store
        .index_node(WHAT_DRIVES_PLATES)
        .unwrap()
        .unwrap();
    assert_eq!(node.read_at, Some(later));
    assert_eq!(node.parent, Some(CHAPTER_MOTION));
    assert_eq!(node.position, 1);
    let tree = fixture.store.index_tree(course).unwrap();
    assert!(tree.chapters[1].sections[1].read);
    assert_eq!(
        fixture
            .store
            .course_record(course)
            .unwrap()
            .unwrap()
            .last_read,
        Some(WHAT_DRIVES_PLATES)
    );
}

// Anchors.

#[test]
fn an_anchor_stays_with_its_own_version() {
    let fixture = Fixture::new();
    let (course, _) = store_sample(&fixture.store);
    let crust = anchor(LAYERS_V1, "crust");
    let entry = fixture
        .store
        .add_glossary_entry(&NewGlossaryEntry {
            course,
            term: "crust".to_owned(),
            definition: "The thin rocky outer layer.".to_owned(),
            anchor: crust.clone(),
        })
        .unwrap();
    assert_eq!(entry.section_id, LAYERS);
    assert_eq!(entry.section_title, "Layers of the Earth");
    let chat = fixture.store.start_side_chat(course, Some(&crust)).unwrap();
    let unanchored = fixture.store.start_side_chat(course, None).unwrap();
    assert_eq!(unanchored.anchor, None);

    let second = Uuid::new_v4();
    fixture
        .store
        .insert_version(&new_version(second, LAYERS, simple_record("crust")), &[])
        .unwrap();
    assert_eq!(
        fixture.store.current_version(LAYERS).unwrap().unwrap().id,
        second
    );

    assert_eq!(
        fixture.store.glossary_for_version(LAYERS_V1).unwrap(),
        vec![entry.clone()]
    );
    assert_eq!(fixture.store.glossary_for_version(second).unwrap(), vec![]);
    assert_eq!(
        fixture.store.glossary_entry_for_anchor(&crust).unwrap(),
        Some(entry.clone())
    );
    assert_eq!(fixture.store.glossary(course).unwrap(), vec![entry.clone()]);
    assert_eq!(fixture.store.glossary(course).unwrap()[0].anchor, crust);

    assert_eq!(
        fixture.store.side_chats_for_version(LAYERS_V1).unwrap(),
        vec![chat.clone()]
    );
    assert_eq!(
        fixture.store.side_chats_for_version(second).unwrap(),
        vec![]
    );
    let chats = fixture.store.side_chats(course).unwrap();
    assert_eq!(chats.len(), 2);
    assert!(chats.contains(&chat));
    assert_eq!(fixture.store.side_chat(chat.id).unwrap(), Some(chat));
}

#[test]
fn a_glossary_entry_keeps_its_wikipedia_result_and_a_chat_its_messages() {
    let fixture = Fixture::new();
    let (course, _) = store_sample(&fixture.store);
    let entry = fixture
        .store
        .add_glossary_entry(&NewGlossaryEntry {
            course,
            term: "mantle".to_owned(),
            definition: "Hot rock below the crust.".to_owned(),
            anchor: anchor(LAYERS_V1, "mantle"),
        })
        .unwrap();
    assert_eq!(entry.wikipedia, None);
    let found = WikipediaResult::Found {
        title: "Mantle (geology)".to_owned(),
        extract: "A layer between the crust and the core.".to_owned(),
        url: "https://en.wikipedia.org/wiki/Mantle_(geology)".to_owned(),
    };
    fixture.store.set_wikipedia(entry.id, &found).unwrap();
    assert_eq!(
        fixture
            .store
            .glossary_entry(entry.id)
            .unwrap()
            .unwrap()
            .wikipedia,
        Some(found)
    );

    let chat = fixture.store.start_side_chat(course, None).unwrap();
    let asked = fixture
        .store
        .add_chat_message(
            chat.id,
            &NewChatMessage {
                role: ChatRole::Candidate,
                text: "Why does the mantle flow?".to_owned(),
                state: MessageState::Complete,
                citations: Vec::new(),
                sources: Vec::new(),
            },
        )
        .unwrap();
    let reply = fixture
        .store
        .add_chat_message(
            chat.id,
            &NewChatMessage {
                role: ChatRole::Agent,
                text: "Because it is hot and".to_owned(),
                state: MessageState::Stopped,
                citations: Vec::new(),
                sources: vec![Source {
                    number: 1,
                    url: "https://example.org/mantle".to_owned(),
                    title: "Mantle".to_owned(),
                }],
            },
        )
        .unwrap();
    assert_eq!(asked.role, ChatRole::Candidate);
    assert_eq!(reply.state, MessageState::Stopped);
    assert_eq!(
        fixture.store.chat_messages(chat.id).unwrap(),
        vec![asked, reply]
    );
}

// Tombstones.

#[test]
fn deleting_any_record_sets_its_tombstone_and_lists_skip_it() {
    let fixture = Fixture::new();
    let (course, sample) = store_sample(&fixture.store);
    let store = &fixture.store;
    let entry = store
        .add_glossary_entry(&NewGlossaryEntry {
            course,
            term: "crust".to_owned(),
            definition: "The thin rocky outer layer.".to_owned(),
            anchor: anchor(LAYERS_V1, "crust"),
        })
        .unwrap();
    let chat = store
        .start_side_chat(course, Some(&anchor(LAYERS_V1, "crust")))
        .unwrap();
    let message = store
        .add_chat_message(
            chat.id,
            &NewChatMessage {
                role: ChatRole::Candidate,
                text: "What is under the crust?".to_owned(),
                state: MessageState::Complete,
                citations: Vec::new(),
                sources: Vec::new(),
            },
        )
        .unwrap();
    let question = KeyedPlacementQuestion {
        id: Uuid::new_v4(),
        rung: RungId(1),
        stem: "What is the crust?".to_owned(),
        options: Vec::new(),
        key: 0,
    };
    store.add_placement_question(course, &question).unwrap();
    let check = store.add_check_answer(LAYERS_V1, 1, 0, 0, true).unwrap();
    let raw = fixture.raw();
    let deleted_at = fixture.advance();

    // Deletes `id` of `kind` from `table`, then checks the row is kept with
    // its id and creation time and carries the deletion time.
    let delete = |kind: RecordKind, table: &str, id: Uuid| {
        let before = stamps(&raw, table, id).expect("the row exists");
        assert!(store.delete(kind, id).unwrap(), "{kind:?} {id}");
        let after = stamps(&raw, table, id).expect("the row is kept");
        assert_eq!(after.0, before.0);
        assert_eq!(after.1, before.1, "creation time kept");
        assert_eq!(after.2, deleted_at, "the deletion is the last update");
        assert_eq!(after.3, Some(deleted_at));
        // A second deletion finds nothing live.
        assert!(!store.delete(kind, id).unwrap());
    };

    delete(RecordKind::ChatMessage, "side_chat_messages", message.id);
    assert_eq!(store.chat_messages(chat.id).unwrap(), vec![]);
    delete(RecordKind::SideChat, "side_chats", chat.id);
    assert_eq!(store.side_chats(course).unwrap(), vec![]);
    assert_eq!(store.side_chat(chat.id).unwrap(), None);
    assert_eq!(store.side_chats_for_version(LAYERS_V1).unwrap(), vec![]);

    delete(RecordKind::GlossaryEntry, "glossary_entries", entry.id);
    assert_eq!(store.glossary(course).unwrap(), vec![]);
    assert_eq!(store.glossary_entry(entry.id).unwrap(), None);
    assert_eq!(store.glossary_for_version(LAYERS_V1).unwrap(), vec![]);

    delete(RecordKind::CheckAnswer, "check_answers", check.id);
    assert_eq!(store.check_answers(LAYERS_V1).unwrap(), vec![]);
    assert_eq!(store.check_answer(LAYERS_V1, 1, 0).unwrap(), None);

    delete(
        RecordKind::PlacementQuestion,
        "placement_questions",
        question.id,
    );
    assert_eq!(store.placement_questions(course).unwrap(), vec![]);

    let convention = any_id(&raw, "conventions");
    delete(RecordKind::Convention, "conventions", convention);
    assert_eq!(
        store.conventions(course).unwrap(),
        sample.conventions[1..].to_vec()
    );

    let rung = any_id(&raw, "ladder_rungs");
    delete(RecordKind::LadderRung, "ladder_rungs", rung);
    assert_eq!(
        store.ladder(course).unwrap().unwrap().rungs,
        sample.ladder.rungs[1..].to_vec()
    );

    let profile_rung = any_id(&raw, "profile_rungs");
    delete(RecordKind::ProfileRung, "profile_rungs", profile_rung);
    let misconception = any_id(&raw, "misconceptions");
    delete(RecordKind::Misconception, "misconceptions", misconception);
    let profile = store.profile(course).unwrap().unwrap();
    assert_eq!(profile.rungs, sample.profile.rungs[1..].to_vec());
    assert_eq!(profile.misconceptions, vec![]);

    // The map entry of the first section, then one of its concepts' links,
    // then a concept.
    let entry_id = any_id(&raw, "map_entries");
    delete(RecordKind::MapEntry, "map_entries", entry_id);
    assert_eq!(store.map_entry(LAYERS).unwrap(), None);
    let link = any_id(&raw, "map_entry_concepts");
    delete(RecordKind::MapEntryConcept, "map_entry_concepts", link);
    let concept = any_id(&raw, "concepts");
    delete(RecordKind::Concept, "concepts", concept);
    let map = store.course_map(course).unwrap();
    assert_eq!(map.concepts, sample.map.concepts[1..].to_vec());
    assert_eq!(map.entries.len(), sample.map.entries.len() - 1);
    assert!(map.entries.iter().all(|entry| entry.section != LAYERS));

    let second = Uuid::new_v4();
    store
        .insert_version(&new_version(second, LAYERS, simple_record("core")), &[])
        .unwrap();
    delete(RecordKind::SectionVersion, "section_versions", second);
    assert_eq!(
        store
            .versions(LAYERS)
            .unwrap()
            .into_iter()
            .map(|version| version.id)
            .collect::<Vec<_>>(),
        vec![LAYERS_V1]
    );
    assert_eq!(store.version(second).unwrap(), None);
    assert_eq!(store.current_version(LAYERS).unwrap(), None);
    assert_eq!(store.current_record(LAYERS).unwrap(), None);

    delete(RecordKind::IndexNode, "index_nodes", MICROPLATES);
    assert_eq!(store.index_node(MICROPLATES).unwrap(), None);
    let tree = store.index_tree(course).unwrap();
    let plate = &tree.chapters[1].sections[0];
    assert_eq!(plate.children.len(), 2);
    assert!(plate.children.iter().all(|child| child.id != MICROPLATES));

    delete(RecordKind::Course, "courses", course);
    assert_eq!(store.course_record(course).unwrap(), None);
    assert!(store.course_records().unwrap().is_empty());
}

// Identity and time.

#[test]
fn every_record_has_a_uuid_and_both_times_from_the_clock() {
    let fixture = Fixture::new();
    let (course, _) = store_sample(&fixture.store);
    let store = &fixture.store;
    let entry = store
        .add_glossary_entry(&NewGlossaryEntry {
            course,
            term: "crust".to_owned(),
            definition: "The thin rocky outer layer.".to_owned(),
            anchor: anchor(LAYERS_V1, "crust"),
        })
        .unwrap();
    let chat = store
        .start_side_chat(course, Some(&anchor(LAYERS_V1, "crust")))
        .unwrap();
    store
        .add_chat_message(
            chat.id,
            &NewChatMessage {
                role: ChatRole::Candidate,
                text: "What is under the crust?".to_owned(),
                state: MessageState::Complete,
                citations: Vec::new(),
                sources: Vec::new(),
            },
        )
        .unwrap();
    let question = KeyedPlacementQuestion {
        id: Uuid::new_v4(),
        rung: RungId(1),
        stem: "What is the crust?".to_owned(),
        options: Vec::new(),
        key: 0,
    };
    store.add_placement_question(course, &question).unwrap();
    store.add_check_answer(LAYERS_V1, 1, 0, 0, true).unwrap();

    let raw = fixture.raw();
    let tables = syncable_tables(&raw);
    for table in [
        "courses",
        "ladder_rungs",
        "profile_rungs",
        "misconceptions",
        "placement_questions",
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
    ] {
        assert!(tables.iter().any(|name| name == table), "missing {table}");
    }
    for table in &tables {
        let rows: Vec<(String, String, String, Option<String>)> = raw
            .prepare(&format!(
                "SELECT id, created_at, updated_at, deleted_at FROM {table}"
            ))
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!rows.is_empty(), "{table} holds a record");
        for (id, created, updated, deleted) in rows {
            Uuid::parse_str(&id).unwrap_or_else(|error| panic!("{table} id {id:?}: {error}"));
            assert_eq!(parse_time(&created), start(), "{table} {id}");
            assert_eq!(parse_time(&updated), start(), "{table} {id}");
            assert_eq!(deleted, None, "{table} {id}");
        }
    }

    // Updates move the update time only.
    let later = fixture.advance();
    let second = Uuid::new_v4();
    store
        .insert_version(&new_version(second, LAYERS, simple_record("core")), &[])
        .unwrap();
    store.set_current_version(LAYERS, LAYERS_V1).unwrap();
    store.mark_opened(course).unwrap();
    store
        .set_wikipedia(entry.id, &WikipediaResult::NoArticle)
        .unwrap();
    store
        .answer_placement_question(question.id, PlacementAnswer::NotSure)
        .unwrap();
    let mut profile = store.profile(course).unwrap().unwrap();
    profile.rungs[7].state = Knowledge::Shaky;
    store.save_profile(course, &profile).unwrap();
    let profile_rung: Uuid = raw
        .query_row("SELECT id FROM profile_rungs WHERE rung = 8", [], |row| {
            row.get::<_, String>(0)
        })
        .map(|id| Uuid::parse_str(&id).unwrap())
        .unwrap();

    for (table, id) in [
        ("index_nodes", LAYERS),
        ("courses", course),
        ("glossary_entries", entry.id),
        ("placement_questions", question.id),
        ("profile_rungs", profile_rung),
    ] {
        let (_, created, updated, deleted) = stamps(&raw, table, id).unwrap();
        assert_eq!(created, start(), "{table} creation time kept");
        assert_eq!(updated, later, "{table} update time moved");
        assert_eq!(deleted, None);
    }
    // Rows the updates did not touch keep both times.
    let (_, created, updated, _) = stamps(&raw, "index_nodes", LITHOSPHERE).unwrap();
    assert_eq!((created, updated), (start(), start()));
    let record = store.course_record(course).unwrap().unwrap();
    assert_eq!(
        (record.created_at, record.updated_at, record.last_opened_at),
        (start(), later, Some(later))
    );
}

// Local settings.

#[test]
fn local_settings_persist_and_are_not_syncable_records() {
    let fixture = Fixture::new();
    assert_eq!(fixture.store.selected_model().unwrap(), None);
    fixture.store.set_selected_model(Some("gpt-5")).unwrap();
    let host = fixture.store.host_id().unwrap();
    assert_eq!(fixture.store.host_id().unwrap(), host);
    drop(fixture.store);

    let store = Store::open(&fixture.path, fixture.clock.clone()).unwrap();
    assert_eq!(store.selected_model().unwrap().as_deref(), Some("gpt-5"));
    assert_eq!(store.host_id().unwrap(), host);
    store.set_selected_model(None).unwrap();
    assert_eq!(store.selected_model().unwrap(), None);

    // Nothing about this machine is in a syncable table.
    let raw = Connection::open(&fixture.path).unwrap();
    for table in syncable_tables(&raw) {
        let columns: Vec<String> = raw
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            !columns.iter().any(|column| column.contains("host")),
            "{table} has {columns:?}"
        );
    }
}
