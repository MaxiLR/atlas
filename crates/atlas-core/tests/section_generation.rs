//! Section generation through the core's request handler: a section streamed
//! through the splitter, its blocks validated, its record resolved, the
//! version stored with record and conventions once complete; stop, failed,
//! incomplete and broken streams that store nothing; retry, and one shared
//! generation for two opens.
//!
//! The sample course is laid down through the store's repository API on the
//! same database file in a temp dir. The provider is scripted per task:
//! section text from `fixtures/sections/` streamed in small chunks, or a
//! recorded Responses stream from the same folder replayed through the
//! Responses stream parser. Expected blocks and records are written out by
//! hand, never derived with the code under test.

mod support;

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::CourseMap;
use atlas_core::domain::record::{
    FixedEntry, Promise, RecordStatus, SectionRecord, StoredRecord, TaughtConcept,
};
use atlas_core::harness::{catalog, prompts};
use atlas_core::protocol::{
    Block, BlockContent, BlockKind, ChapterNode, CoreError, CoreEvent, CoreRequest, CoreResponse,
    GenerationEvent, GenerationOutcome, GenerationTarget, Model, NodeMark, SectionView,
    VersionStyle,
};
use atlas_core::provider::chatgpt::ResponsesStreamParser;
use atlas_core::provider::{
    ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, Task,
};
use atlas_core::store::{NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, RecordingEvents};
use futures::StreamExt;
use futures::stream;
use serde_json::{Value, json};
use support::sample_course::{KINDS_OF_BOUNDARY, SampleCourse, WHAT_DRIVES_PLATES, sample_course};
use tempfile::TempDir;
use tokio::sync::Notify;
use uuid::Uuid;

const MODEL: &str = "test-model";

/// In the text of every version laid down, and in no request.
const TEXT_SENTINEL: &str = "sentinel-in-another-section-5d0e";
/// In the summary and the concept line of the record of `complete.md`.
const RECORD_SENTINELS: [&str; 2] = ["sentinel-summary-71c3", "sentinel-concept-4be2"];

/// The first paragraph of every section fixture.
const FIRST_PARAGRAPH: &str = "The plates do not drift on their own. Heat escaping from deep \
    inside the Earth keeps the mantle moving, and the plates ride on top of that motion.";
const SECOND_PARAGRAPH: &str = "Hot rock rises beneath the ridges and cooler rock sinks at the \
    trenches. Where the cold edge of a plate sinks, its weight drags the rest of the plate \
    behind it.";

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
    _dir: TempDir,
    provider: Arc<ScriptedProvider>,
    events: Arc<RecordingEvents>,
    core: Core,
    /// The same database file, for laying the course down and reading back.
    store: Store,
    course: Uuid,
}

impl Harness {
    /// The sample course laid down, its map as given.
    fn with_map(map: impl FnOnce(CourseMap) -> CourseMap) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(ScriptedProvider::default());
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        let mut sample = sample_course();
        sample.map = map(sample.map);
        let course = store_course(&store, &sample);
        events.take();
        Self {
            _dir: dir,
            provider,
            events,
            core,
            store,
            course,
        }
    }

    fn new() -> Self {
        Self::with_map(|map| map)
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

    async fn retry(&self, section_id: Uuid) -> Result<SectionView, CoreError> {
        self.send(CoreRequest::RetrySection {
            course_id: self.course,
            section_id,
        })
        .await
        .map(section)
    }

    async fn stop(&self, section_id: Uuid) {
        assert_eq!(
            self.send(CoreRequest::StopGeneration {
                course_id: self.course,
                section_id,
            })
            .await,
            Ok(CoreResponse::Done)
        );
    }

    fn section_requests(&self) -> Vec<ProviderRequest> {
        self.provider.requests_for(Task::WriteSection)
    }

    fn read(&self, node: Uuid) -> bool {
        self.store
            .index_node(node)
            .unwrap()
            .expect("the node exists")
            .read_at
            .is_some()
    }

    /// Asserts that nothing of a generation of `node` was stored.
    fn assert_nothing_stored(&self, node: Uuid, conventions_before: &[Convention]) {
        assert_eq!(self.store.versions(node).unwrap(), vec![]);
        assert_eq!(self.store.current_version(node).unwrap(), None);
        assert_eq!(self.store.current_record(node).unwrap(), None);
        assert_eq!(
            self.store.conventions(self.course).unwrap(),
            conventions_before
        );
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
                &prose_version(
                    record.version,
                    record.section,
                    &format!("{TEXT_SENTINEL}: the text of version {}.", record.version),
                    record.record.clone(),
                ),
                &fixed_by(ConventionOrigin::SectionVersion {
                    version: record.version,
                }),
            )
            .unwrap();
    }
    course
}

fn prose_version(id: Uuid, node: Uuid, text: &str, record: StoredRecord) -> NewSectionVersion {
    NewSectionVersion {
        id,
        node,
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
        record,
        model: Some("gpt-5".to_owned()),
        prompt: "write-section@1".to_owned(),
    }
}

// -- reading the events ----------------------------------------------------------

/// The generation events of `node`, in order.
fn generation_events(events: &[CoreEvent], node: Uuid) -> Vec<GenerationEvent> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::Generation {
                target: GenerationTarget::Section { section_id, .. },
                event,
                ..
            } if *section_id == node => Some(event.clone()),
            _ => None,
        })
        .collect()
}

/// One step of a generation with prose deltas left out.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Started,
    Opened(u32, BlockKind),
    Completed(u32, BlockKind),
    Finished(GenerationOutcome),
}

fn kind_of(content: &BlockContent) -> BlockKind {
    match content {
        BlockContent::Prose { .. } => BlockKind::Prose,
        BlockContent::Visual { .. } | BlockContent::Fallback { .. } => BlockKind::Visual,
        BlockContent::Figure { .. } => BlockKind::Figure,
        BlockContent::Mermaid { .. } => BlockKind::Mermaid,
        BlockContent::Check { .. } => BlockKind::Check,
    }
}

fn steps(events: &[GenerationEvent]) -> Vec<Step> {
    events
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::Started => Some(Step::Started),
            GenerationEvent::BlockOpened { block, kind } => Some(Step::Opened(*block, *kind)),
            GenerationEvent::BlockCompleted { block } => {
                Some(Step::Completed(block.index, kind_of(&block.content)))
            }
            GenerationEvent::Finished { outcome } => Some(Step::Finished(outcome.clone())),
            GenerationEvent::TextDelta { .. }
            | GenerationEvent::Activity { .. }
            | GenerationEvent::CitationAdded { .. } => None,
        })
        .collect()
}

/// The prose that arrived for `block`.
fn arrived(events: &[GenerationEvent], block: u32) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::TextDelta { block: at, text } if *at == block => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn completed_blocks(events: &[GenerationEvent]) -> Vec<Block> {
    events
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::BlockCompleted { block } => Some(block.clone()),
            _ => None,
        })
        .collect()
}

fn outcome(events: &[GenerationEvent]) -> Option<GenerationOutcome> {
    events.iter().rev().find_map(|event| match event {
        GenerationEvent::Finished { outcome } => Some(outcome.clone()),
        _ => None,
    })
}

/// The chapters of the last index-changed event of the course.
fn index_changed(events: &[CoreEvent], course: Uuid) -> Option<Vec<ChapterNode>> {
    events.iter().rev().find_map(|event| match event {
        CoreEvent::IndexChanged {
            course_id,
            chapters,
        } if *course_id == course => Some(chapters.clone()),
        _ => None,
    })
}

/// Whether `node`, at any depth, is shown read.
fn shown_read(chapters: &[ChapterNode], node: Uuid) -> Option<bool> {
    fn find(sections: &[atlas_core::protocol::SectionNode], node: Uuid) -> Option<bool> {
        sections.iter().find_map(|section| {
            if section.id == node {
                Some(section.read && section.mark == NodeMark::Read)
            } else {
                find(&section.deep_dives, node)
            }
        })
    }
    chapters
        .iter()
        .find_map(|chapter| find(&chapter.sections, node))
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

/// The record `complete.md` closes with.
fn complete_record() -> SectionRecord {
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

/// The record of `repaired-record.json`.
fn repaired_record() -> SectionRecord {
    SectionRecord {
        summary: Some(
            "The plates are moved by the convecting mantle beneath them. Heat escaping from the \
             interior keeps it moving."
                .to_owned(),
        ),
        concepts: vec![taught(
            "mantle convection",
            "Heat escaping from deep inside the Earth keeps the mantle moving.",
        )],
        notation: Vec::new(),
        terms: Vec::new(),
        examples: Vec::new(),
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

fn callout(body: &str) -> Value {
    json!({
        "root": "note",
        "elements": {
            "note": {
                "type": "callout",
                "props": { "title": "The mantle is not liquid", "body": body },
                "children": []
            }
        }
    })
}

const VISUAL_ALT: &str = "A callout: the mantle is solid rock that flows very slowly";

fn prose_text(block: &Block) -> &str {
    match &block.content {
        BlockContent::Prose { text } => text,
        other => panic!("expected prose, got {other:?}"),
    }
}

// -- a complete section ----------------------------------------------------------

#[tokio::test]
async fn a_complete_stream_streams_its_blocks_and_stores_one_version_with_its_record() {
    let harness = Harness::new();
    harness
        .provider
        .script(Task::WriteSection, Script::Complete(fixture("complete.md")));

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let events = harness.events.take();
    let generation = generation_events(&events, WHAT_DRIVES_PLATES);
    assert_eq!(
        steps(&generation),
        [
            Step::Started,
            Step::Opened(0, BlockKind::Prose),
            Step::Completed(0, BlockKind::Prose),
            Step::Opened(1, BlockKind::Visual),
            Step::Completed(1, BlockKind::Visual),
            Step::Opened(2, BlockKind::Prose),
            Step::Completed(2, BlockKind::Prose),
            Step::Opened(3, BlockKind::Figure),
            Step::Completed(3, BlockKind::Figure),
            Step::Opened(4, BlockKind::Check),
            Step::Completed(4, BlockKind::Check),
            Step::Finished(GenerationOutcome::Completed),
        ]
    );

    // Prose arrives as text between its opening and its completion.
    let blocks = completed_blocks(&generation);
    assert_eq!(arrived(&generation, 0).trim(), FIRST_PARAGRAPH);
    assert_eq!(prose_text(&blocks[0]).trim(), FIRST_PARAGRAPH);
    assert_eq!(arrived(&generation, 2).trim(), SECOND_PARAGRAPH);
    assert_eq!(prose_text(&blocks[2]).trim(), SECOND_PARAGRAPH);
    let first_delta = generation
        .iter()
        .position(|event| matches!(event, GenerationEvent::TextDelta { block: 0, .. }))
        .expect("prose arrives");
    let opened_at = generation
        .iter()
        .position(|event| matches!(event, GenerationEvent::BlockOpened { block: 0, .. }))
        .unwrap();
    assert!(opened_at < first_delta);

    assert_eq!(
        blocks[1].content,
        BlockContent::Visual {
            spec: callout(
                "Mantle rock is solid, yet over millions of years it flows like very stiff putty."
            ),
            description: Some(VISUAL_ALT.to_owned()),
        }
    );
    match &blocks[3].content {
        BlockContent::Figure { svg, description } => {
            assert_eq!(
                description.as_deref(),
                Some("A loop of rising and sinking mantle beneath two plates")
            );
            assert!(svg.as_str().contains("M20 80C60 20 140 20 180 80"));
        }
        other => panic!("expected a figure, got {other:?}"),
    }
    match &blocks[4].content {
        BlockContent::Check { questions } => {
            assert_eq!(questions.len(), 1);
            assert_eq!(questions[0].stem, "What keeps the plates moving?");
            assert_eq!(
                questions[0].options,
                [
                    "Heat escaping from the Earth's interior",
                    "The Moon's pull on the crust",
                    "Winds blowing over the oceans",
                    "Nothing: the plates are still",
                ]
            );
            assert_eq!(questions[0].answer, None);
        }
        other => panic!("expected check questions, got {other:?}"),
    }

    // One version, current, with the blocks shown and the stream's record.
    let versions = harness.store.versions(WHAT_DRIVES_PLATES).unwrap();
    assert_eq!(versions.len(), 1);
    let stored = &versions[0];
    assert_eq!(
        harness.store.current_version(WHAT_DRIVES_PLATES).unwrap(),
        Some(stored.clone())
    );
    assert_eq!(stored.blocks, blocks);
    assert_eq!(stored.style, VersionStyle::Original);
    assert_eq!(
        stored.record,
        Some(StoredRecord {
            record: complete_record(),
            status: RecordStatus::Confirmed,
        })
    );
    assert_eq!(stored.model.as_deref(), Some(MODEL));
    assert_eq!(stored.prompt, prompts::WRITE_SECTION.id());
    assert_eq!(stored.checks.len(), 1);
    assert_eq!(stored.checks[0].block, 4);
    assert_eq!(stored.checks[0].questions[0].key, 0);

    assert_eq!(opened.id, WHAT_DRIVES_PLATES);
    assert_eq!(opened.course_id, harness.course);
    assert_eq!(opened.title, "What drives the plates");
    assert_eq!(opened.version.id, stored.id);
    assert_eq!(opened.version.blocks, blocks);
    assert_eq!(opened.version.model.as_deref(), Some(MODEL));
    assert_eq!(opened.versions.len(), 1);

    // Reopening answers from the store.
    let requests = harness.provider.requests().len();
    let reopened = harness.open(WHAT_DRIVES_PLATES).await.expect("stored");
    assert_eq!(harness.provider.requests().len(), requests);
    assert_eq!(reopened, opened);
    assert_eq!(harness.events.take(), []);
}

#[tokio::test]
async fn no_event_or_response_carries_any_part_of_the_record() {
    let harness = Harness::new();
    harness
        .provider
        .script(Task::WriteSection, Script::Complete(fixture("complete.md")));

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let events = harness.events.take();
    assert!(!events.is_empty());
    let mut shown: Vec<String> = events
        .iter()
        .map(|event| serde_json::to_string(event).unwrap())
        .collect();
    shown.push(serde_json::to_string(&opened).unwrap());
    for text in &shown {
        for sentinel in RECORD_SENTINELS {
            assert!(!text.contains(sentinel), "{sentinel} reached: {text}");
        }
        assert!(
            !text.contains("atlas-record"),
            "the record block reached: {text}"
        );
        assert!(
            !text.contains("slab pull"),
            "a record entry reached: {text}"
        );
    }
    // The record was stored all the same.
    let record = harness
        .store
        .current_record(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("a record");
    assert!(
        record
            .record
            .summary
            .is_some_and(|summary| summary.contains(RECORD_SENTINELS[0]))
    );
}

#[tokio::test]
async fn the_request_carries_the_catalog_the_conventions_the_entry_and_the_records_relied_on() {
    let harness = Harness::new();
    harness
        .provider
        .script(Task::WriteSection, Script::Complete(fixture("complete.md")));

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let requests = harness.section_requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.model.as_deref(), Some(MODEL));
    assert!(
        request.instructions.contains(catalog::prompt()),
        "the catalog's instructions"
    );
    assert!(request.instructions.contains("atlas-record"));
    assert!(request.instructions.contains("atlas-check"));
    let input = &request.input;
    // The conventions, the plan's and a section's.
    assert!(input.contains("plate velocity, in centimetres per year"));
    assert!(input.contains("the widening Atlantic Ocean"));
    assert!(input.contains("the weak, slowly flowing upper mantle beneath the lithosphere"));
    // The section's own map entry.
    assert!(input.contains("What drives the plates"));
    assert!(input.contains("mantle convection"));
    assert!(input.contains("mantle, asthenosphere, tectonic plate"));
    // The records of the sections introducing what it relies on.
    assert!(input.contains("The lithosphere is broken into plates that move as rigid pieces."));
    assert!(input.contains("By strength, the crust and the top of the mantle form one rigid"));
    assert!(input.contains("The iron centre, liquid outside and solid inside."));
    // What was promised to it.
    assert!(input.contains("what makes the plates move"));
    // No other section's text.
    for text in [&request.instructions, input] {
        assert!(
            !text.contains(TEXT_SENTINEL),
            "another section's text: {text}"
        );
    }
}

// -- the record ------------------------------------------------------------------

#[tokio::test]
async fn an_invalid_record_is_asked_for_once_more_and_stored_confirmed_when_it_holds() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("invalid-record.md")),
    );
    harness.provider.script(
        Task::RepairRecord,
        Script::Complete(fixture("repaired-record.json")),
    );

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let repairs = harness.provider.requests_for(Task::RepairRecord);
    assert_eq!(repairs.len(), 1);
    assert!(
        repairs[0].input.contains(FIRST_PARAGRAPH),
        "the section's text"
    );
    assert_eq!(
        harness.store.current_record(WHAT_DRIVES_PLATES).unwrap(),
        Some(StoredRecord {
            record: repaired_record(),
            status: RecordStatus::Confirmed,
        })
    );
}

#[tokio::test]
async fn a_record_invalid_twice_stores_the_predicted_record_unconfirmed() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("invalid-record.md")),
    );
    harness.provider.script(
        Task::RepairRecord,
        Script::Complete("Still not a record.".to_owned()),
    );

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.provider.requests_for(Task::RepairRecord).len(), 1);
    assert_eq!(harness.provider.requests().len(), 2);
    assert_eq!(
        harness.store.current_record(WHAT_DRIVES_PLATES).unwrap(),
        Some(predicted_record())
    );
    assert_eq!(opened.version.blocks.len(), 1);
    assert_eq!(
        prose_text(&opened.version.blocks[0]).trim(),
        FIRST_PARAGRAPH
    );
}

#[tokio::test]
async fn a_record_missing_from_the_stream_and_the_repair_stores_the_predicted_record() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("no-record.md")),
    );
    harness
        .provider
        .script(Task::RepairRecord, Script::Complete(String::new()));

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.provider.requests_for(Task::RepairRecord).len(), 1);
    assert_eq!(
        harness.store.current_record(WHAT_DRIVES_PLATES).unwrap(),
        Some(predicted_record())
    );
    assert!(harness.read(WHAT_DRIVES_PLATES));
}

#[tokio::test]
async fn a_section_without_a_map_entry_is_written_from_an_empty_entry() {
    let harness = Harness::with_map(|mut map| {
        map.entries
            .retain(|entry| entry.section != WHAT_DRIVES_PLATES);
        map
    });
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("no-record.md")),
    );
    harness
        .provider
        .script(Task::RepairRecord, Script::Complete(String::new()));

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert!(
        harness.section_requests()[0]
            .input
            .contains("nothing planned")
    );
    assert_eq!(
        harness.store.current_record(WHAT_DRIVES_PLATES).unwrap(),
        Some(StoredRecord {
            record: SectionRecord {
                summary: None,
                concepts: Vec::new(),
                notation: Vec::new(),
                terms: Vec::new(),
                examples: Vec::new(),
                promises: Vec::new(),
            },
            status: RecordStatus::Predicted,
        })
    );
}

// -- conventions -----------------------------------------------------------------

#[tokio::test]
async fn a_new_term_is_stored_as_a_convention_of_this_version() {
    let harness = Harness::new();
    harness
        .provider
        .script(Task::WriteSection, Script::Complete(fixture("complete.md")));

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

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
                version: opened.version.id,
            },
        }]
    );
}

#[tokio::test]
async fn a_redefined_symbol_keeps_its_first_fixing_and_leaves_the_stored_record() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("redefines-symbol.md")),
    );

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let conventions = harness.store.conventions(harness.course).unwrap();
    let v: Vec<_> = conventions
        .iter()
        .filter(|convention| convention.kind == ConventionKind::Notation && convention.key == "v")
        .collect();
    assert_eq!(
        v,
        [&Convention {
            kind: ConventionKind::Notation,
            key: "v".to_owned(),
            meaning: "plate velocity, in centimetres per year".to_owned(),
            origin: ConventionOrigin::Plan,
        }]
    );
    assert!(
        !conventions
            .iter()
            .any(|convention| convention.meaning.contains("volume of the mantle"))
    );
    // The new term is fixed by this version.
    assert!(conventions.iter().any(|convention| {
        convention.key == "ridge push"
            && convention.origin
                == ConventionOrigin::SectionVersion {
                    version: opened.version.id,
                }
    }));
    let record = harness
        .store
        .current_record(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("a record");
    assert_eq!(record.status, RecordStatus::Confirmed);
    assert_eq!(record.record.notation, vec![]);
    assert_eq!(
        record.record.terms,
        vec![fixed(
            "ridge push",
            "the shove a plate gets from the high ground of a ridge"
        )]
    );
}

// -- visuals ---------------------------------------------------------------------

#[tokio::test]
async fn an_invalid_visual_is_repaired_once() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("invalid-visual.md")),
    );
    harness.provider.script(
        Task::RepairBlock,
        Script::Complete(fixture("repaired-visual.json")),
    );

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.provider.requests_for(Task::RepairBlock).len(), 1);
    let expected = BlockContent::Visual {
        spec: callout("Mantle rock is solid, yet it flows like very stiff putty."),
        description: Some(VISUAL_ALT.to_owned()),
    };
    assert_eq!(opened.version.blocks[1].content, expected);
    let stored = harness
        .store
        .current_version(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("stored");
    assert_eq!(stored.blocks[1].content, expected);
}

#[tokio::test]
async fn a_visual_invalid_twice_is_stored_as_its_text_fallback() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Complete(fixture("invalid-visual.md")),
    );
    harness.provider.script(
        Task::RepairBlock,
        Script::Complete(r#"{"root": "note"}"#.to_owned()),
    );

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.provider.requests_for(Task::RepairBlock).len(), 1);
    let events = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    let fallback = BlockContent::Fallback {
        description: VISUAL_ALT.to_owned(),
    };
    assert_eq!(completed_blocks(&events)[1].content, fallback);
    let stored = harness
        .store
        .current_version(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("stored");
    assert_eq!(stored.blocks[1].content, fallback);
}

// -- stop, retry, failures -------------------------------------------------------

#[tokio::test]
async fn stop_ends_the_stream_at_once_and_stores_nothing_then_retry_stores() {
    let harness = Harness::new();
    let text = fixture("complete.md");
    let (first, rest) = text.split_at(text.find("```atlas-visual").unwrap());
    harness.provider.script(
        Task::WriteSection,
        Script::Held(first.to_owned(), rest.to_owned()),
    );
    let conventions = harness.store.conventions(harness.course).unwrap();

    let (opened, ()) = tokio::join!(harness.open(WHAT_DRIVES_PLATES), async {
        harness.provider.held.notified().await;
        harness.stop(WHAT_DRIVES_PLATES).await;
    });

    assert!(opened.is_err(), "nothing to show: {opened:?}");
    let events = harness.events.take();
    let generation = generation_events(&events, WHAT_DRIVES_PLATES);
    assert_eq!(outcome(&generation), Some(GenerationOutcome::Stopped));
    assert_eq!(arrived(&generation, 0).trim(), FIRST_PARAGRAPH);
    assert_eq!(harness.provider.requests().len(), 1, "no repair request");
    harness.assert_nothing_stored(WHAT_DRIVES_PLATES, &conventions);
    assert!(!harness.read(WHAT_DRIVES_PLATES));
    assert_eq!(index_changed(&events, harness.course), None);

    harness
        .provider
        .script(Task::WriteSection, Script::Complete(text.clone()));
    let retried = harness.retry(WHAT_DRIVES_PLATES).await.expect("written");

    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    assert_eq!(outcome(&generation), Some(GenerationOutcome::Completed));
    assert_eq!(harness.section_requests().len(), 2);
    let versions = harness.store.versions(WHAT_DRIVES_PLATES).unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].id, retried.version.id);
    assert_eq!(retried.version.blocks.len(), 5);
    assert!(harness.read(WHAT_DRIVES_PLATES));
}

#[tokio::test]
async fn a_plan_limit_ends_the_generation_failed_after_one_request_and_stores_nothing() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Recorded(fixture("plan-limit.sse")),
    );
    let conventions = harness.store.conventions(harness.course).unwrap();

    let opened = harness.open(WHAT_DRIVES_PLATES).await;

    assert_eq!(opened, Err(CoreError::PlanLimit { resets_at: None }));
    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    assert_eq!(
        outcome(&generation),
        Some(GenerationOutcome::Failed {
            error: CoreError::PlanLimit { resets_at: None },
        })
    );
    assert_eq!(
        arrived(&generation, 0),
        "The plates do not drift on their own. Heat escaping from deep inside the Earth "
    );
    assert_eq!(harness.provider.requests().len(), 1);
    harness.assert_nothing_stored(WHAT_DRIVES_PLATES, &conventions);
    assert!(!harness.read(WHAT_DRIVES_PLATES));
}

#[tokio::test]
async fn an_incomplete_stream_ends_incomplete_and_stores_nothing() {
    let harness = Harness::new();
    harness.provider.script(
        Task::WriteSection,
        Script::Recorded(fixture("incomplete.sse")),
    );
    let conventions = harness.store.conventions(harness.course).unwrap();

    let opened = harness.open(WHAT_DRIVES_PLATES).await;

    assert!(opened.is_err(), "{opened:?}");
    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    assert_eq!(
        outcome(&generation),
        Some(GenerationOutcome::Incomplete {
            reason: Some("max_output_tokens".to_owned()),
        })
    );
    assert!(arrived(&generation, 0).starts_with("The plates do not drift"));
    assert_eq!(harness.provider.requests().len(), 1);
    harness.assert_nothing_stored(WHAT_DRIVES_PLATES, &conventions);
    assert!(!harness.read(WHAT_DRIVES_PLATES));
}

#[tokio::test]
async fn a_broken_stream_ends_failed_and_stores_nothing() {
    let harness = Harness::new();
    harness
        .provider
        .script(Task::WriteSection, Script::Recorded(fixture("broken.sse")));
    let conventions = harness.store.conventions(harness.course).unwrap();

    let opened = harness.open(WHAT_DRIVES_PLATES).await;

    assert!(
        matches!(opened, Err(CoreError::Provider { .. })),
        "{opened:?}"
    );
    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    assert!(
        matches!(
            outcome(&generation),
            Some(GenerationOutcome::Failed {
                error: CoreError::Provider { .. }
            })
        ),
        "{generation:?}"
    );
    assert!(arrived(&generation, 0).starts_with("The plates do not drift"));
    assert_eq!(harness.provider.requests().len(), 1);
    harness.assert_nothing_stored(WHAT_DRIVES_PLATES, &conventions);
    assert!(!harness.read(WHAT_DRIVES_PLATES));
}

// -- reading ---------------------------------------------------------------------

#[tokio::test]
async fn a_section_is_read_once_its_generation_completes() {
    let harness = Harness::new();
    harness
        .provider
        .script(Task::WriteSection, Script::Complete(fixture("complete.md")));
    assert!(!harness.read(WHAT_DRIVES_PLATES));

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert!(harness.read(WHAT_DRIVES_PLATES));
    let events = harness.events.take();
    let chapters = index_changed(&events, harness.course).expect("the index changed");
    assert_eq!(shown_read(&chapters, WHAT_DRIVES_PLATES), Some(true));
    // The course resumes at it.
    match harness
        .send(CoreRequest::OpenCourse {
            course_id: harness.course,
        })
        .await
    {
        Ok(CoreResponse::Course { course }) => {
            assert_eq!(course.resume_at, Some(WHAT_DRIVES_PLATES));
            assert_eq!(course.chapters, chapters);
        }
        other => panic!("expected the course, got {other:?}"),
    }
}

#[tokio::test]
async fn opening_a_stored_version_never_opened_marks_it_read_with_no_request() {
    let harness = Harness::new();
    let version = Uuid::from_u128(0x1301);
    harness
        .store
        .insert_version(
            &prose_version(
                version,
                KINDS_OF_BOUNDARY,
                "Plates meet in three ways.",
                predicted_record(),
            ),
            &[],
        )
        .unwrap();
    assert!(!harness.read(KINDS_OF_BOUNDARY));

    let opened = harness.open(KINDS_OF_BOUNDARY).await.expect("stored");

    assert!(harness.provider.requests().is_empty());
    assert_eq!(opened.version.id, version);
    assert_eq!(
        prose_text(&opened.version.blocks[0]),
        "Plates meet in three ways."
    );
    assert!(harness.read(KINDS_OF_BOUNDARY));
    let events = harness.events.take();
    let chapters = index_changed(&events, harness.course).expect("the index changed");
    assert_eq!(shown_read(&chapters, KINDS_OF_BOUNDARY), Some(true));

    // Opened again it is already read: nothing more is reported.
    harness.open(KINDS_OF_BOUNDARY).await.expect("stored");
    assert_eq!(harness.events.take(), []);
}

// -- one generation per section --------------------------------------------------

#[tokio::test]
async fn two_concurrent_opens_share_one_generation() {
    let harness = Harness::new();
    let text = fixture("complete.md");
    let (first, rest) = text.split_at(text.find("```atlas-visual").unwrap());
    harness.provider.script(
        Task::WriteSection,
        Script::Held(first.to_owned(), rest.to_owned()),
    );

    let (one, two, ()) = tokio::join!(
        harness.open(WHAT_DRIVES_PLATES),
        harness.open(WHAT_DRIVES_PLATES),
        async {
            harness.provider.held.notified().await;
            for _ in 0..8 {
                tokio::task::yield_now().await;
            }
            harness.provider.release.notify_one();
        }
    );

    let one = one.expect("written");
    assert_eq!(two.expect("written"), one);
    assert_eq!(harness.section_requests().len(), 1);
    assert_eq!(harness.store.versions(WHAT_DRIVES_PLATES).unwrap().len(), 1);
    let events = harness.events.take();
    let started = generation_events(&events, WHAT_DRIVES_PLATES)
        .into_iter()
        .filter(|event| *event == GenerationEvent::Started)
        .count();
    assert_eq!(started, 1);
}
