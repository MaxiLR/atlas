//! Explain differently and the choice of version through the core's request
//! handler: new versions in each style, each stored with its own record and
//! made current, the earlier ones kept untouched; the request built with the
//! style's instruction, the section's context and the current version's
//! record; conventions kept as first fixed; later sections given the record
//! of whichever version is current; a version selected with no request and
//! kept across reopening; stopped and failed attempts storing nothing; and
//! anchors kept to their own version.
//!
//! The sample course is laid down through the store's repository API on the
//! same database file in a temp dir; "What drives the plates" is first
//! written from `fixtures/sections/complete.md`. The provider is scripted per
//! task. Expected blocks and records are written out by hand, never derived
//! with the code under test.

mod support;

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::record::{
    FixedEntry, Promise, RecordStatus, SectionRecord, StoredRecord, TaughtConcept,
};
use atlas_core::protocol::{
    Anchor, Block, BlockContent, CoreError, CoreEvent, CoreRequest, CoreResponse, ExplainStyle,
    GenerationEvent, GenerationOutcome, GenerationTarget, GlossaryEntry, Model, SectionView,
    VersionStyle,
};
use atlas_core::provider::chatgpt::ResponsesStreamParser;
use atlas_core::provider::{
    ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, Task,
};
use atlas_core::store::{NewSectionVersion, Store, StoredSectionVersion};
use atlas_core::testing::{FakePorts, RecordingEvents};
use futures::StreamExt;
use futures::stream;
use support::sample_course::{
    KINDS_OF_BOUNDARY, LAYERS, LAYERS_V1, SampleCourse, WHAT_DRIVES_PLATES, sample_course,
};
use tempfile::TempDir;
use tokio::sync::Notify;
use uuid::Uuid;

const MODEL: &str = "test-model";

/// In the text of every version laid down, and in no request.
const TEXT_SENTINEL: &str = "sentinel-in-another-section-5d0e";
/// In the summary and the concept line of the record of `complete.md`, the
/// original version of "What drives the plates".
const ORIGINAL_SENTINELS: [&str; 2] = ["sentinel-summary-71c3", "sentinel-concept-4be2"];
/// In the summary of the record of the simpler version.
const SIMPLER_SENTINEL: &str = "sentinel-simpler-9a1f";
/// In the summary of the record of the more technical version.
const TECHNICAL_SENTINEL: &str = "sentinel-technical-3c7e";

/// The first paragraph of `complete.md`.
const FIRST_PARAGRAPH: &str = "The plates do not drift on their own. Heat escaping from deep \
    inside the Earth keeps the mantle moving, and the plates ride on top of that motion.";

// The instruction `prompts/explain-differently.md` gives for each style.
const SIMPLER_INSTRUCTION: &str = "Explain it more simply.";
const TECHNICAL_INSTRUCTION: &str =
    "Explain it more technically, for a reader ready for precision.";
const EXAMPLES_INSTRUCTION: &str = "Explain it through more examples.";
// What it asks of every new version.
const SAME_CONCEPTS: &str = "Teach the same planned concepts";
const KEEP_CONVENTIONS: &str = "Keep the course's conventions.";

const SIMPLER_PROSE: &str = "Deep inside the Earth it is very hot. Warm rock rises slowly, cool \
    rock sinks, and the plates on top are carried along.";
const TECHNICAL_PROSE: &str = "Heat flux from the core and radiogenic heating drive thermal \
    convection of the solid mantle over millions of years.";
const EXAMPLES_PROSE: &str = "Think of a lava lamp: warm wax rises, cools and sinks again. The \
    mantle turns over the same way, only far more slowly.";
const REDEFINING_PROSE: &str = "The sinking edge of a plate draws the mantle down behind it.";

const SIMPLER_RECORD: &str = r#"{"summary": "Hot rock deep in the Earth slowly rises and cool rock sinks. sentinel-simpler-9a1f The plates ride along on top.", "concepts": [{"name": "mantle convection", "line": "Warm rock rises, cool rock sinks, and the plates are carried along."}], "notation": [], "terms": [], "examples": ["warm air rising above a heater"], "promises": []}"#;
const TECHNICAL_RECORD: &str = r#"{"summary": "Mantle convection is thermal convection of solid rock. sentinel-technical-3c7e It is driven by heat from the core and by radioactive decay.", "concepts": [{"name": "mantle convection", "line": "Thermal convection of the solid mantle over millions of years."}], "notation": [], "terms": [{"key": "radiogenic heat", "meaning": "heat released by the decay of radioactive elements in the rock"}], "examples": [], "promises": []}"#;
const EXAMPLES_RECORD: &str = r#"{"summary": "The mantle turns over like the wax in a lava lamp. The plates ride on that slow motion.", "concepts": [{"name": "mantle convection", "line": "Rock rises where it is warm and sinks where it is cool, like wax in a lava lamp."}], "notation": [], "terms": [], "examples": ["a lava lamp"], "promises": []}"#;
/// Gives "slab pull", fixed by the original version, another meaning, and
/// fixes a new term.
const REDEFINING_RECORD: &str = r#"{"summary": "A sinking plate draws the mantle down behind it. Ridges push the plates apart.", "concepts": [{"name": "mantle convection", "line": "The mantle moves with the plates that sink into it."}], "notation": [], "terms": [{"key": "slab pull", "meaning": "the suction of the mantle beneath a sinking plate"}, {"key": "ridge push", "meaning": "the shove a plate gets from the high ground of a ridge"}], "examples": [], "promises": []}"#;
/// A record without a summary.
const INVALID_RECORD: &str =
    r#"{"concepts": [{"name": "mantle convection", "line": "Warm rock rises."}]}"#;

/// A section's text closed by its record.
fn explained(prose: &str, record: &str) -> String {
    format!("{prose}\n\n```atlas-record\n{record}\n```\n")
}

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sections")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

// -- the scripted provider -------------------------------------------------------

/// What one request is answered with.
enum Script {
    /// The text in small chunks, then completed.
    Complete(String),
    /// The first text in small chunks, then a hold until released, then the
    /// rest and completed.
    Held(String, String),
    /// A recorded Responses stream, parsed as it would arrive.
    Recorded(String),
    /// The request fails before anything streams.
    Failed,
}

/// Answers each task with its scripts in order, records every request, and
/// reports when a held stream reaches its hold.
#[derive(Default)]
struct ScriptedProvider {
    scripts: Mutex<Vec<(Task, VecDeque<Script>)>>,
    requests: Mutex<Vec<ProviderRequest>>,
    held: Arc<Notify>,
    release: Arc<Notify>,
}

impl ScriptedProvider {
    fn script(&self, task: Task, script: Script) {
        let mut scripts = self.scripts.lock().unwrap();
        match scripts.iter_mut().find(|(scripted, _)| *scripted == task) {
            Some((_, queue)) => queue.push_back(script),
            None => scripts.push((task, VecDeque::from([script]))),
        }
    }

    fn requests(&self) -> Vec<ProviderRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn requests_for(&self, task: Task) -> Vec<ProviderRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.task == task)
            .collect()
    }
}

fn chunks(text: &str) -> Vec<Result<ProviderEvent, ProviderError>> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(11)
        .map(|chars| Ok(ProviderEvent::TextDelta(chars.iter().collect())))
        .collect()
}

fn replay(recording: &str) -> Vec<Result<ProviderEvent, ProviderError>> {
    let mut parser = ResponsesStreamParser::new(Some("req_section_fixture".to_owned()));
    let mut events = Vec::new();
    for chunk in recording.as_bytes().chunks(7) {
        events.extend(parser.feed(chunk));
    }
    events.extend(parser.finish());
    events
        .into_iter()
        .map(|event| event.into_provider())
        .collect()
}

#[async_trait]
impl ModelProvider for ScriptedProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let task = request.task;
        self.requests.lock().unwrap().push(request);
        let script = self
            .scripts
            .lock()
            .unwrap()
            .iter_mut()
            .find(|(scripted, _)| *scripted == task)
            .and_then(|(_, queue)| queue.pop_front())
            .ok_or_else(|| ProviderError::failed(format!("no script for {task:?}")))?;
        let completed = || stream::iter([Ok(ProviderEvent::Completed)]);
        Ok(match script {
            Script::Complete(text) => stream::iter(chunks(&text)).chain(completed()).boxed(),
            Script::Recorded(recording) => stream::iter(replay(&recording)).boxed(),
            Script::Held(first, rest) => {
                let (held, release) = (self.held.clone(), self.release.clone());
                let hold = stream::once(async move {
                    held.notify_one();
                    release.notified().await;
                    stream::iter(chunks(&rest))
                })
                .flatten();
                stream::iter(chunks(&first))
                    .chain(hold)
                    .chain(completed())
                    .boxed()
            }
            Script::Failed => return Err(ProviderError::failed("the scripted request failed")),
        })
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        Ok(vec![Model {
            slug: MODEL.to_owned(),
            display_name: "Test model".to_owned(),
        }])
    }
}

// -- the harness -----------------------------------------------------------------

struct Harness {
    dir: TempDir,
    provider: Arc<ScriptedProvider>,
    events: Arc<RecordingEvents>,
    core: Core,
    /// The same database file, for laying the course down and reading back.
    store: Store,
    course: Uuid,
}

impl Harness {
    /// The sample course laid down, with nothing of "What drives the plates"
    /// written yet.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(ScriptedProvider::default());
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        let course = store_course(&store, &sample_course());
        events.take();
        Self {
            dir,
            provider,
            events,
            core,
            store,
            course,
        }
    }

    /// The sample course with "What drives the plates" written once from
    /// `complete.md`: its original version.
    async fn with_original() -> (Self, StoredSectionVersion) {
        let harness = Self::new();
        harness
            .provider
            .script(Task::WriteSection, Script::Complete(fixture("complete.md")));
        let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");
        let original = harness
            .store
            .version(opened.version.id)
            .unwrap()
            .expect("stored");
        assert_eq!(
            original.record,
            Some(StoredRecord {
                record: original_record(),
                status: RecordStatus::Confirmed,
            })
        );
        harness.events.take();
        (harness, original)
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("atlas.sqlite3")
    }

    async fn send(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        self.core.handle(request).await
    }

    async fn open(&self, section_id: Uuid) -> Result<SectionView, CoreError> {
        self.send(CoreRequest::OpenSection {
            course_id: self.course,
            section_id,
        })
        .await
        .map(section)
    }

    async fn explain(&self, style: ExplainStyle) -> Result<SectionView, CoreError> {
        self.send(CoreRequest::ExplainDifferently {
            course_id: self.course,
            section_id: WHAT_DRIVES_PLATES,
            style,
        })
        .await
        .map(section)
    }

    /// Scripts the explanation's stream, then asks for it.
    async fn explain_with(&self, style: ExplainStyle, text: String) -> SectionView {
        self.provider
            .script(Task::ExplainDifferently, Script::Complete(text));
        self.explain(style).await.expect("explained")
    }

    async fn select(&self, section_id: Uuid, version_id: Uuid) -> Result<SectionView, CoreError> {
        self.send(CoreRequest::SelectVersion {
            course_id: self.course,
            section_id,
            version_id,
        })
        .await
        .map(section)
    }

    async fn stop(&self) {
        assert_eq!(
            self.send(CoreRequest::StopGeneration {
                course_id: self.course,
                section_id: WHAT_DRIVES_PLATES,
            })
            .await,
            Ok(CoreResponse::Done)
        );
    }

    fn explain_requests(&self) -> Vec<ProviderRequest> {
        self.provider.requests_for(Task::ExplainDifferently)
    }

    fn versions(&self) -> Vec<StoredSectionVersion> {
        self.store.versions(WHAT_DRIVES_PLATES).unwrap()
    }

    fn current(&self) -> StoredSectionVersion {
        self.store
            .current_version(WHAT_DRIVES_PLATES)
            .unwrap()
            .expect("a current version")
    }
}

fn section(response: CoreResponse) -> SectionView {
    match response {
        CoreResponse::Section { section } => section,
        other => panic!("expected a section, got {other:?}"),
    }
}

/// Lays the course down: ladder, profile, index with its map and the plan's
/// conventions, then a version for every record with the conventions it
/// fixed, each version's text carrying the sentinel.
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
                &NewSectionVersion {
                    id: record.version,
                    node: record.section,
                    style: VersionStyle::Original,
                    blocks: vec![Block {
                        index: 0,
                        content: BlockContent::Prose {
                            text: format!(
                                "{TEXT_SENTINEL}: the text of version {}.",
                                record.version
                            ),
                        },
                    }],
                    checks: Vec::new(),
                    citations: Vec::new(),
                    sources: Vec::new(),
                    written_without_search: false,
                    record: record.record.clone(),
                    model: Some("gpt-5".to_owned()),
                    prompt: "write-section@1".to_owned(),
                },
                &fixed_by(ConventionOrigin::SectionVersion {
                    version: record.version,
                }),
            )
            .unwrap();
    }
    course
}

/// The generation events of "What drives the plates", with their target.
fn generation_events(events: &[CoreEvent]) -> Vec<(VersionStyle, GenerationEvent)> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::Generation {
                target:
                    GenerationTarget::Section {
                        section_id, style, ..
                    },
                event,
                ..
            } if *section_id == WHAT_DRIVES_PLATES => Some((*style, event.clone())),
            _ => None,
        })
        .collect()
}

fn outcome(events: &[(VersionStyle, GenerationEvent)]) -> Option<GenerationOutcome> {
    events.iter().rev().find_map(|(_, event)| match event {
        GenerationEvent::Finished { outcome } => Some(outcome.clone()),
        _ => None,
    })
}

// -- expected values -------------------------------------------------------------

fn taught(name: &str, line: &str) -> TaughtConcept {
    TaughtConcept {
        name: name.to_owned(),
        line: Some(line.to_owned()),
    }
}

fn fixed(key: &str, meaning: &str) -> FixedEntry {
    FixedEntry {
        key: key.to_owned(),
        meaning: meaning.to_owned(),
    }
}

fn confirmed(record: SectionRecord) -> Option<StoredRecord> {
    Some(StoredRecord {
        record,
        status: RecordStatus::Confirmed,
    })
}

/// The record `complete.md` closes with.
fn original_record() -> SectionRecord {
    SectionRecord {
        summary: Some(
            "The plates are moved by the mantle beneath them, which slowly convects as heat \
             escapes. sentinel-summary-71c3 The sinking edge of a plate drags the rest along."
                .to_owned(),
        ),
        concepts: vec![taught(
            "mantle convection",
            "Hot mantle rises and cool mantle sinks, sentinel-concept-4be2, carrying the plates.",
        )],
        notation: Vec::new(),
        terms: vec![fixed(
            "slab pull",
            "the drag of a sinking plate edge on the rest of its plate",
        )],
        examples: vec!["a pot of soup simmering on a stove".to_owned()],
        promises: vec![Promise {
            section: "3.1".to_owned(),
            what: "what happens where two plates meet".to_owned(),
        }],
    }
}

fn simpler_record() -> SectionRecord {
    SectionRecord {
        summary: Some(
            "Hot rock deep in the Earth slowly rises and cool rock sinks. sentinel-simpler-9a1f \
             The plates ride along on top."
                .to_owned(),
        ),
        concepts: vec![taught(
            "mantle convection",
            "Warm rock rises, cool rock sinks, and the plates are carried along.",
        )],
        notation: Vec::new(),
        terms: Vec::new(),
        examples: vec!["warm air rising above a heater".to_owned()],
        promises: Vec::new(),
    }
}

fn technical_record() -> SectionRecord {
    SectionRecord {
        summary: Some(
            "Mantle convection is thermal convection of solid rock. sentinel-technical-3c7e It \
             is driven by heat from the core and by radioactive decay."
                .to_owned(),
        ),
        concepts: vec![taught(
            "mantle convection",
            "Thermal convection of the solid mantle over millions of years.",
        )],
        notation: Vec::new(),
        terms: vec![fixed(
            "radiogenic heat",
            "heat released by the decay of radioactive elements in the rock",
        )],
        examples: Vec::new(),
        promises: Vec::new(),
    }
}

fn examples_record() -> SectionRecord {
    SectionRecord {
        summary: Some(
            "The mantle turns over like the wax in a lava lamp. The plates ride on that slow \
             motion."
                .to_owned(),
        ),
        concepts: vec![taught(
            "mantle convection",
            "Rock rises where it is warm and sinks where it is cool, like wax in a lava lamp.",
        )],
        notation: Vec::new(),
        terms: Vec::new(),
        examples: vec!["a lava lamp".to_owned()],
        promises: Vec::new(),
    }
}

/// What the map entry of "What drives the plates" predicts.
fn predicted_record() -> StoredRecord {
    StoredRecord {
        record: SectionRecord {
            summary: None,
            concepts: vec![TaughtConcept {
                name: "mantle convection".to_owned(),
                line: None,
            }],
            notation: Vec::new(),
            terms: Vec::new(),
            examples: Vec::new(),
            promises: Vec::new(),
        },
        status: RecordStatus::Predicted,
    }
}

fn prose_of(blocks: &[Block]) -> Vec<&str> {
    blocks
        .iter()
        .map(|block| match &block.content {
            BlockContent::Prose { text } => text.trim(),
            other => panic!("expected prose, got {other:?}"),
        })
        .collect()
}

// -- new versions ----------------------------------------------------------------

#[tokio::test]
async fn one_request_per_style_keeps_four_versions_in_order_with_the_latest_current() {
    let (harness, original) = Harness::with_original().await;
    let mut before = vec![original];

    let steps = [
        (
            ExplainStyle::Simpler,
            VersionStyle::Simpler,
            SIMPLER_PROSE,
            SIMPLER_RECORD,
            simpler_record(),
        ),
        (
            ExplainStyle::MoreTechnical,
            VersionStyle::MoreTechnical,
            TECHNICAL_PROSE,
            TECHNICAL_RECORD,
            technical_record(),
        ),
        (
            ExplainStyle::MoreExamples,
            VersionStyle::MoreExamples,
            EXAMPLES_PROSE,
            EXAMPLES_RECORD,
            examples_record(),
        ),
    ];
    for (asked, style, prose, record, expected_record) in steps {
        let view = harness.explain_with(asked, explained(prose, record)).await;

        // The new version is shown and current, with its own record.
        assert_eq!(view.version.style, style);
        assert_eq!(prose_of(&view.version.blocks), [prose]);
        let current = harness.current();
        assert_eq!(current.id, view.version.id);
        assert_eq!(current.style, style);
        assert_eq!(current.record, confirmed(expected_record));
        // The earlier versions and their records are as they were.
        let versions = harness.versions();
        assert_eq!(versions[..versions.len() - 1], before[..]);
        assert_eq!(versions.last(), Some(&current));
        before = versions;
    }

    let styles: Vec<_> = before.iter().map(|version| version.style).collect();
    assert_eq!(
        styles,
        [
            VersionStyle::Original,
            VersionStyle::Simpler,
            VersionStyle::MoreTechnical,
            VersionStyle::MoreExamples,
        ]
    );
    // The view lists them in the order made, with their styles.
    let shown = harness.open(WHAT_DRIVES_PLATES).await.expect("stored");
    let listed: Vec<_> = shown
        .versions
        .iter()
        .map(|version| (version.id, version.style))
        .collect();
    let stored: Vec<_> = before
        .iter()
        .map(|version| (version.id, version.style))
        .collect();
    assert_eq!(listed, stored);
    assert_eq!(shown.version.id, before[3].id);
    assert_eq!(prose_of(&shown.version.blocks), [EXAMPLES_PROSE]);
    assert_eq!(harness.explain_requests().len(), 3);
    // Each request carried the record of the version current when it was made.
    let last = &harness.explain_requests()[2];
    assert!(last.input.contains(TECHNICAL_SENTINEL), "{}", last.input);
    assert!(!last.input.contains(SIMPLER_SENTINEL), "{}", last.input);
}

#[tokio::test]
async fn a_new_version_streams_as_its_style_and_completes() {
    let (harness, _) = Harness::with_original().await;

    harness
        .explain_with(
            ExplainStyle::Simpler,
            explained(SIMPLER_PROSE, SIMPLER_RECORD),
        )
        .await;

    let generation = generation_events(&harness.events.take());
    assert_eq!(
        generation.first().map(|(_, event)| event),
        Some(&GenerationEvent::Started)
    );
    assert!(
        generation
            .iter()
            .all(|(style, _)| *style == VersionStyle::Simpler),
        "{generation:?}"
    );
    assert_eq!(outcome(&generation), Some(GenerationOutcome::Completed));
}

// -- the request -----------------------------------------------------------------

#[tokio::test]
async fn the_request_carries_the_style_the_conventions_the_entry_and_the_current_record() {
    let (harness, _) = Harness::with_original().await;

    harness
        .explain_with(
            ExplainStyle::Simpler,
            explained(SIMPLER_PROSE, SIMPLER_RECORD),
        )
        .await;

    let requests = harness.explain_requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.model.as_deref(), Some(MODEL));
    // The section's own instructions, and the style's.
    assert!(request.instructions.contains("atlas-record"));
    assert!(request.instructions.contains(SIMPLER_INSTRUCTION));
    assert!(request.instructions.contains(SAME_CONCEPTS));
    assert!(request.instructions.contains(KEEP_CONVENTIONS));
    assert!(!request.instructions.contains(TECHNICAL_INSTRUCTION));
    assert!(!request.instructions.contains(EXAMPLES_INSTRUCTION));
    let input = &request.input;
    // The conventions, the plan's, a section's and the original version's.
    assert!(input.contains("plate velocity, in centimetres per year"));
    assert!(input.contains("the widening Atlantic Ocean"));
    assert!(input.contains("the weak, slowly flowing upper mantle beneath the lithosphere"));
    assert!(input.contains("the drag of a sinking plate edge on the rest of its plate"));
    // The section's own map entry.
    assert!(input.contains("What drives the plates"));
    assert!(input.contains("mantle, asthenosphere, tectonic plate"));
    // The records of the sections introducing what it relies on.
    assert!(input.contains("The lithosphere is broken into plates that move as rigid pieces."));
    // The current version's record.
    for sentinel in ORIGINAL_SENTINELS {
        assert!(input.contains(sentinel), "the current record: {input}");
    }
    // No other section's text.
    for text in [&request.instructions, input] {
        assert!(
            !text.contains(TEXT_SENTINEL),
            "another section's text: {text}"
        );
    }

    // The next request carries the record of the version now current.
    harness
        .explain_with(
            ExplainStyle::MoreTechnical,
            explained(TECHNICAL_PROSE, TECHNICAL_RECORD),
        )
        .await;
    let request = &harness.explain_requests()[1];
    assert!(request.instructions.contains(TECHNICAL_INSTRUCTION));
    assert!(!request.instructions.contains(SIMPLER_INSTRUCTION));
    assert!(request.input.contains(SIMPLER_SENTINEL));
    for sentinel in ORIGINAL_SENTINELS {
        assert!(
            !request.input.contains(sentinel),
            "only the current record: {}",
            request.input
        );
    }
}

#[tokio::test]
async fn a_section_never_written_has_nothing_to_explain_differently() {
    let harness = Harness::new();

    let explained = harness.explain(ExplainStyle::Simpler).await;

    assert!(
        matches!(explained, Err(CoreError::InvalidRequest { .. })),
        "{explained:?}"
    );
    assert!(harness.provider.requests().is_empty());
    assert_eq!(harness.versions(), vec![]);
}

// -- records and conventions -----------------------------------------------------

#[tokio::test]
async fn an_invalid_record_in_the_new_version_is_repaired_once_then_predicted_for_it_alone() {
    let (harness, original) = Harness::with_original().await;
    harness.provider.script(
        Task::RepairRecord,
        Script::Complete("Still not a record.".to_owned()),
    );

    let view = harness
        .explain_with(
            ExplainStyle::Simpler,
            explained(SIMPLER_PROSE, INVALID_RECORD),
        )
        .await;

    let repairs = harness.provider.requests_for(Task::RepairRecord);
    assert_eq!(repairs.len(), 1);
    assert!(repairs[0].input.contains(SIMPLER_PROSE), "its own text");
    assert!(
        !repairs[0].input.contains(FIRST_PARAGRAPH),
        "not the original's"
    );
    let new = harness
        .store
        .version(view.version.id)
        .unwrap()
        .expect("stored");
    assert_eq!(new.record, Some(predicted_record()));
    assert_eq!(harness.current().id, new.id);
    // The original keeps its own confirmed record.
    assert_eq!(harness.store.version(original.id).unwrap(), Some(original));
}

#[tokio::test]
async fn a_new_version_redefining_a_term_the_original_fixed_leaves_it_as_first_fixed() {
    let (harness, original) = Harness::with_original().await;

    let view = harness
        .explain_with(
            ExplainStyle::MoreExamples,
            explained(REDEFINING_PROSE, REDEFINING_RECORD),
        )
        .await;

    let conventions = harness.store.conventions(harness.course).unwrap();
    let slab_pull: Vec<_> = conventions
        .iter()
        .filter(|convention| convention.key == "slab pull")
        .collect();
    assert_eq!(
        slab_pull,
        [&Convention {
            kind: ConventionKind::Terminology,
            key: "slab pull".to_owned(),
            meaning: "the drag of a sinking plate edge on the rest of its plate".to_owned(),
            origin: ConventionOrigin::SectionVersion {
                version: original.id,
            },
        }]
    );
    assert!(
        !conventions
            .iter()
            .any(|convention| convention.meaning.contains("suction")),
        "{conventions:?}"
    );
    // The new term is fixed by the new version, whose record keeps it alone.
    assert!(conventions.iter().any(|convention| {
        convention.key == "ridge push"
            && convention.origin
                == ConventionOrigin::SectionVersion {
                    version: view.version.id,
                }
    }));
    let record = harness.current().record.expect("a record");
    assert_eq!(
        record.record.terms,
        vec![fixed(
            "ridge push",
            "the shove a plate gets from the high ground of a ridge"
        )]
    );
}

#[tokio::test]
async fn the_next_section_carries_the_record_of_whichever_version_is_current() {
    let (harness, original) = Harness::with_original().await;
    harness
        .explain_with(
            ExplainStyle::Simpler,
            explained(SIMPLER_PROSE, SIMPLER_RECORD),
        )
        .await;

    // "Kinds of boundary" follows it and relies on what it introduces.
    harness.provider.script(Task::WriteSection, Script::Failed);
    harness
        .open(KINDS_OF_BOUNDARY)
        .await
        .expect_err("the scripted request fails");
    let request = harness
        .provider
        .requests_for(Task::WriteSection)
        .pop()
        .unwrap();
    assert!(
        request.input.contains(SIMPLER_SENTINEL),
        "{}",
        request.input
    );
    for sentinel in ORIGINAL_SENTINELS {
        assert!(!request.input.contains(sentinel), "{}", request.input);
    }

    harness
        .select(WHAT_DRIVES_PLATES, original.id)
        .await
        .expect("selected");
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(explained(
            "Plates meet in three ways.",
            r#"{"summary": "Plates meet in three ways. They pull apart, collide or slide past each other.", "concepts": [{"name": "divergent boundary", "line": "Where plates pull apart."}], "notation": [], "terms": [], "examples": [], "promises": []}"#,
        )),
    );
    harness.open(KINDS_OF_BOUNDARY).await.expect("written");
    let request = harness
        .provider
        .requests_for(Task::WriteSection)
        .pop()
        .unwrap();
    for sentinel in ORIGINAL_SENTINELS {
        assert!(request.input.contains(sentinel), "{}", request.input);
    }
    assert!(
        !request.input.contains(SIMPLER_SENTINEL),
        "{}",
        request.input
    );
}

// -- selecting a version ---------------------------------------------------------

#[tokio::test]
async fn selecting_the_first_version_makes_no_request_and_survives_reopening_the_store() {
    let (harness, original) = Harness::with_original().await;
    let simpler = harness
        .explain_with(
            ExplainStyle::Simpler,
            explained(SIMPLER_PROSE, SIMPLER_RECORD),
        )
        .await;
    harness.events.take();
    let requests = harness.provider.requests().len();

    let selected = harness
        .select(WHAT_DRIVES_PLATES, original.id)
        .await
        .expect("selected");

    assert_eq!(harness.provider.requests().len(), requests, "no request");
    assert_eq!(generation_events(&harness.events.take()), []);
    assert_eq!(selected.version.id, original.id);
    assert_eq!(selected.version.style, VersionStyle::Original);
    assert_eq!(selected.version.blocks, original.blocks);
    assert_eq!(prose_of(&selected.version.blocks[..1]), [FIRST_PARAGRAPH]);
    let listed: Vec<_> = selected.versions.iter().map(|version| version.id).collect();
    assert_eq!(listed, [original.id, simpler.version.id]);
    assert_eq!(harness.current().id, original.id);
    assert_eq!(harness.versions().len(), 2, "nothing added");

    // Reopened, the store and a new core both have the original current.
    let path = harness.path();
    let reopened = Store::open(&path, FakePorts::default().clock.clone()).unwrap();
    assert_eq!(
        reopened
            .current_version(WHAT_DRIVES_PLATES)
            .unwrap()
            .map(|version| version.id),
        Some(original.id)
    );
    let ports = FakePorts::default();
    let core = Core::new(ports.deps(
        path,
        harness.provider.clone(),
        Arc::new(RecordingEvents::default()),
    ))
    .expect("core starts");
    let opened = section(
        core.handle(CoreRequest::OpenSection {
            course_id: harness.course,
            section_id: WHAT_DRIVES_PLATES,
        })
        .await
        .expect("opened"),
    );
    assert_eq!(opened.version.id, original.id);
    assert_eq!(opened.version.blocks, original.blocks);
    assert_eq!(harness.provider.requests().len(), requests, "no request");

    // And back to the newer one.
    let back = harness
        .select(WHAT_DRIVES_PLATES, simpler.version.id)
        .await
        .expect("selected");
    assert_eq!(prose_of(&back.version.blocks), [SIMPLER_PROSE]);
    assert_eq!(harness.current().id, simpler.version.id);
}

#[tokio::test]
async fn a_version_of_another_section_cannot_be_selected() {
    let (harness, original) = Harness::with_original().await;

    let selected = harness.select(WHAT_DRIVES_PLATES, LAYERS_V1).await;

    assert!(
        matches!(selected, Err(CoreError::NotFound { .. })),
        "{selected:?}"
    );
    assert_eq!(harness.current().id, original.id);
    assert_eq!(
        harness
            .store
            .current_version(LAYERS)
            .unwrap()
            .map(|version| version.id),
        Some(LAYERS_V1)
    );
    assert!(
        harness
            .provider
            .requests_for(Task::ExplainDifferently)
            .is_empty()
    );
}

// -- stop and failure ------------------------------------------------------------

#[tokio::test]
async fn a_stopped_or_failed_attempt_adds_no_version_and_no_record() {
    let (harness, original) = Harness::with_original().await;
    let conventions = harness.store.conventions(harness.course).unwrap();
    let assert_unchanged = |harness: &Harness| {
        assert_eq!(harness.versions(), vec![original.clone()]);
        assert_eq!(harness.current(), original);
        assert_eq!(
            harness.store.conventions(harness.course).unwrap(),
            conventions
        );
    };

    // Stopped while it streams.
    let text = explained(SIMPLER_PROSE, SIMPLER_RECORD);
    let (first, rest) = text.split_at(text.find("```atlas-record").unwrap());
    harness.provider.script(
        Task::ExplainDifferently,
        Script::Held(first.to_owned(), rest.to_owned()),
    );
    let (stopped, ()) = tokio::join!(harness.explain(ExplainStyle::Simpler), async {
        tokio::time::timeout(Duration::from_secs(10), harness.provider.held.notified())
            .await
            .expect("the explanation streams up to its hold");
        harness.stop().await;
    });
    assert!(stopped.is_err(), "{stopped:?}");
    let generation = generation_events(&harness.events.take());
    assert_eq!(outcome(&generation), Some(GenerationOutcome::Stopped));
    assert_unchanged(&harness);

    // The plan's limit reached mid-stream.
    harness.provider.script(
        Task::ExplainDifferently,
        Script::Recorded(fixture("plan-limit.sse")),
    );
    let limited = harness.explain(ExplainStyle::MoreTechnical).await;
    assert_eq!(limited, Err(CoreError::PlanLimit { resets_at: None }));
    let generation = generation_events(&harness.events.take());
    assert_eq!(
        outcome(&generation),
        Some(GenerationOutcome::Failed {
            error: CoreError::PlanLimit { resets_at: None },
        })
    );
    assert_unchanged(&harness);

    // An incomplete response.
    harness.provider.script(
        Task::ExplainDifferently,
        Script::Recorded(fixture("incomplete.sse")),
    );
    let incomplete = harness.explain(ExplainStyle::MoreExamples).await;
    assert!(incomplete.is_err(), "{incomplete:?}");
    assert_unchanged(&harness);

    // A request that fails at once.
    harness
        .provider
        .script(Task::ExplainDifferently, Script::Failed);
    let failed = harness.explain(ExplainStyle::Simpler).await;
    assert!(
        matches!(failed, Err(CoreError::Provider { .. })),
        "{failed:?}"
    );
    assert_unchanged(&harness);
    assert_eq!(harness.explain_requests().len(), 4);
    assert!(harness.provider.requests_for(Task::RepairRecord).is_empty());
}

// -- anchors ---------------------------------------------------------------------

#[tokio::test]
async fn a_glossary_anchor_on_version_one_is_returned_for_it_and_not_for_version_two() {
    let (harness, original) = Harness::with_original().await;
    let anchor = Anchor {
        version_id: original.id,
        block: 0,
        quote: "mantle".to_owned(),
    };
    harness.provider.script(
        Task::Define,
        Script::Complete("The thick layer of hot rock between the crust and the core.".to_owned()),
    );
    let entry = match harness
        .send(CoreRequest::Define {
            anchor: anchor.clone(),
            sentence: FIRST_PARAGRAPH.to_owned(),
        })
        .await
    {
        Ok(CoreResponse::Definition { entry }) => entry,
        other => panic!("expected a definition, got {other:?}"),
    };

    let simpler = harness
        .explain_with(
            ExplainStyle::Simpler,
            explained(SIMPLER_PROSE, SIMPLER_RECORD),
        )
        .await;

    assert_eq!(
        harness.store.glossary_for_version(original.id).unwrap(),
        vec![entry.clone()]
    );
    assert_eq!(
        harness
            .store
            .glossary_for_version(simpler.version.id)
            .unwrap(),
        Vec::<GlossaryEntry>::new()
    );
    // Asked again on version one, it answers from the store.
    let requests = harness.provider.requests().len();
    let again = harness
        .send(CoreRequest::Define {
            anchor,
            sentence: FIRST_PARAGRAPH.to_owned(),
        })
        .await;
    assert_eq!(again, Ok(CoreResponse::Definition { entry }));
    assert_eq!(harness.provider.requests().len(), requests);
}
