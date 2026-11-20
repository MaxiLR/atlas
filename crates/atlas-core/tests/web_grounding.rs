//! Web-grounded sections through the core's request handler: the section
//! request turns the hosted web search on, the provider's citations (ranges
//! of the whole output text) are placed on the prose blocks they fall in and
//! stored with numbered sources, citation events arrive while the section
//! streams, and a route that refuses search gets the section written once
//! more without it, for the rest of the session.
//!
//! The sample course is laid down through the store's repository API on the
//! same database file in a temp dir. The provider answers each write-section
//! request with a scripted stream of provider events. Offsets and expected
//! citations are written out by hand, never derived with the code under test.

mod support;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionOrigin};
use atlas_core::protocol::{
    Activity, Block, BlockContent, Citation, CoreError, CoreEvent, CoreRequest, CoreResponse,
    GenerationEvent, GenerationOutcome, GenerationTarget, Model, SectionView, Source, VersionStyle,
};
use atlas_core::provider::{
    ModelProvider, ProviderCitation, ProviderError, ProviderEvent, ProviderRequest, ProviderStream,
    Task,
};
use atlas_core::store::{NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, RecordingEvents};
use futures::StreamExt;
use futures::stream;
use support::sample_course::{KINDS_OF_BOUNDARY, SampleCourse, WHAT_DRIVES_PLATES, sample_course};
use tempfile::TempDir;
use uuid::Uuid;

const MODEL: &str = "test-model";

/// Block 0, prose. Its 87 characters hold one em dash, three bytes in UTF-8.
const PROSE_0: &str =
    "Heat escaping from the core drives the mantle — slowly.\nThe plates ride on top of it.\n\n";
/// Block 1, a figure: 202 characters of the output, from 87 to 289.
const FIGURE_1: &str = "```atlas-figure\nalt: A loop of rising and sinking mantle\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" fill=\"none\" stroke=\"currentColor\"><path d=\"M20 80C60 20 140 20 180 80\"/></svg>\n```\n";
/// Block 2, prose: from 289 to 333 of the output.
const PROSE_2: &str = "\nRidges spread apart where hot rock rises.\n\n";
/// The closing record, never shown.
const RECORD: &str = "```atlas-record\n{\"summary\": \"The plates are moved by the mantle beneath them, which slowly convects as heat escapes.\", \"concepts\": [{\"name\": \"mantle convection\", \"line\": \"Hot mantle rises and cool mantle sinks, carrying the plates.\"}], \"notation\": [], \"terms\": [], \"examples\": [], \"promises\": []}\n```\n";

const MANTLE_URL: &str = "https://example.org/mantle";
const MANTLE_TITLE: &str = "Mantle convection";
const PLATES_URL: &str = "https://example.org/plates";
const PLATES_TITLE: &str = "How plates ride";

/// A plain section, no blocks but prose and no record.
const PLAIN: &str = "Plates meet in three ways: they spread, collide or slide past.\n";

// -- the scripted provider -------------------------------------------------------

/// What one write-section request is answered with.
enum Script {
    /// These events, in order.
    Events(Vec<Result<ProviderEvent, ProviderError>>),
    /// The call fails before any event.
    Refused(ProviderError),
}

/// Answers each write-section request with its script in order and records
/// every request. Any other task fails.
#[derive(Default)]
struct ScriptedProvider {
    scripts: Mutex<VecDeque<Script>>,
    requests: Mutex<Vec<ProviderRequest>>,
}

impl ScriptedProvider {
    fn script(&self, script: Script) {
        self.scripts.lock().unwrap().push_back(script);
    }

    fn section_requests(&self) -> Vec<ProviderRequest> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.task == Task::WriteSection)
            .cloned()
            .collect()
    }
}

#[async_trait]
impl ModelProvider for ScriptedProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let task = request.task;
        self.requests.lock().unwrap().push(request);
        if task != Task::WriteSection {
            return Err(ProviderError::failed(format!("no script for {task:?}")));
        }
        match self.scripts.lock().unwrap().pop_front() {
            Some(Script::Events(events)) => Ok(stream::iter(events).boxed()),
            Some(Script::Refused(error)) => Err(error),
            None => Err(ProviderError::failed("no script left")),
        }
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        Ok(vec![Model {
            slug: MODEL.to_owned(),
            display_name: "Test model".to_owned(),
        }])
    }
}

/// `text` as deltas of at most seven characters.
fn deltas(text: &str) -> Vec<Result<ProviderEvent, ProviderError>> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(7)
        .map(|chars| Ok(ProviderEvent::TextDelta(chars.iter().collect())))
        .collect()
}

fn cite(url: &str, title: &str, start: u32, end: u32) -> Result<ProviderEvent, ProviderError> {
    Ok(ProviderEvent::Citation(ProviderCitation {
        url: url.to_owned(),
        title: title.to_owned(),
        start,
        end,
    }))
}

fn completed() -> Result<ProviderEvent, ProviderError> {
    Ok(ProviderEvent::Completed)
}

/// A plain section, completed.
fn plain() -> Script {
    let mut events = deltas(PLAIN);
    events.push(completed());
    Script::Events(events)
}

/// The route refusing the search tool.
fn search_refused() -> ProviderError {
    ProviderError::Unsupported {
        param: Some("tools".to_owned()),
        request_id: Some("req_search".to_owned()),
    }
}

/// The plan's usage limit reached.
fn plan_limit() -> ProviderError {
    ProviderError::PlanLimit {
        resets_at: None,
        request_id: Some("req_limit".to_owned()),
    }
}

/// The grounded section: a search, then the first paragraph and its two
/// citations, the figure, the second paragraph and a third citation to the
/// first source, then the record.
fn grounded() -> Script {
    let mut events = vec![Ok(ProviderEvent::Searching)];
    events.extend(deltas(PROSE_0));
    // "Heat escaping from the core drives the mantle"
    events.push(cite(MANTLE_URL, MANTLE_TITLE, 0, 45));
    // "The plates ride on top of it."
    events.push(cite(PLATES_URL, PLATES_TITLE, 56, 85));
    events.extend(deltas(FIGURE_1));
    events.extend(deltas(PROSE_2));
    // "Ridges spread apart"
    events.push(cite(MANTLE_URL, MANTLE_TITLE, 290, 309));
    events.extend(deltas(RECORD));
    events.push(completed());
    Script::Events(events)
}

// -- the harness -----------------------------------------------------------------

struct Harness {
    _dir: TempDir,
    provider: Arc<ScriptedProvider>,
    events: Arc<RecordingEvents>,
    core: Core,
    store: Store,
    course: Uuid,
}

impl Harness {
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
            _dir: dir,
            provider,
            events,
            core,
            store,
            course,
        }
    }

    async fn open(&self, section_id: Uuid) -> Result<SectionView, CoreError> {
        self.core
            .handle(CoreRequest::OpenSection {
                course_id: self.course,
                section_id,
            })
            .await
            .map(|response| match response {
                CoreResponse::Section { section } => section,
                other => panic!("expected a section, got {other:?}"),
            })
    }

    /// Whether each write-section request so far turned search on.
    fn searches(&self) -> Vec<bool> {
        self.provider
            .section_requests()
            .iter()
            .map(|request| request.web_search)
            .collect()
    }
}

/// Lays the course down: ladder, profile, index with its map and the plan's
/// conventions, then a version for every record with the conventions it
/// fixed.
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
                            text: format!("The text of version {}.", record.version),
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

fn citation_events(events: &[GenerationEvent]) -> Vec<(Citation, Source)> {
    events
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::CitationAdded { citation, source } => {
                Some((citation.clone(), source.clone()))
            }
            _ => None,
        })
        .collect()
}

fn position(events: &[GenerationEvent], what: impl Fn(&GenerationEvent) -> bool) -> usize {
    events.iter().position(what).expect("the event was emitted")
}

fn prose(blocks: &[Block], index: u32) -> String {
    match &blocks
        .iter()
        .find(|block| block.index == index)
        .expect("the block exists")
        .content
    {
        BlockContent::Prose { text } => text.clone(),
        other => panic!("block {index} is not prose: {other:?}"),
    }
}

/// The characters `start..end` of `text`.
fn slice(text: &str, start: u32, end: u32) -> String {
    text.chars()
        .skip(start as usize)
        .take((end - start) as usize)
        .collect()
}

fn citation(block: u32, start: u32, end: u32, source: u32) -> Citation {
    Citation {
        block,
        start,
        end,
        source,
    }
}

fn source(number: u32, url: &str, title: &str) -> Source {
    Source {
        number,
        url: url.to_owned(),
        title: title.to_owned(),
    }
}

fn expected_citations() -> Vec<Citation> {
    vec![
        citation(0, 0, 45, 1),
        citation(0, 56, 85, 2),
        citation(2, 1, 20, 1),
    ]
}

fn expected_sources() -> Vec<Source> {
    vec![
        source(1, MANTLE_URL, MANTLE_TITLE),
        source(2, PLATES_URL, PLATES_TITLE),
    ]
}

// -- search on ------------------------------------------------------------------

#[tokio::test]
async fn a_section_request_turns_the_web_search_on_and_reports_the_search() {
    let harness = Harness::new();
    harness.provider.script(grounded());

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.searches(), [true]);
    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    let searching = position(&generation, |event| {
        matches!(
            event,
            GenerationEvent::Activity {
                activity: Activity::Searching
            }
        )
    });
    let first_text = position(&generation, |event| {
        matches!(event, GenerationEvent::TextDelta { .. })
    });
    assert!(searching < first_text, "{generation:?}");
}

// -- citations ------------------------------------------------------------------

#[tokio::test]
async fn three_citations_to_two_sources_are_stored_on_their_blocks_with_two_numbered_sources() {
    let harness = Harness::new();
    harness.provider.script(grounded());

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let stored = harness
        .store
        .current_version(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("a version is stored");
    assert_eq!(prose(&stored.blocks, 0), PROSE_0);
    assert_eq!(prose(&stored.blocks, 2), PROSE_2);
    assert_eq!(stored.citations, expected_citations());
    assert_eq!(stored.sources, expected_sources());
    assert!(!stored.written_without_search);

    // Each citation covers its claim in its block's own text.
    assert_eq!(
        slice(&prose(&stored.blocks, 0), 0, 45),
        "Heat escaping from the core drives the mantle"
    );
    assert_eq!(
        slice(&prose(&stored.blocks, 0), 56, 85),
        "The plates ride on top of it."
    );
    assert_eq!(
        slice(&prose(&stored.blocks, 2), 1, 20),
        "Ridges spread apart"
    );

    // The answer shows what was stored.
    assert_eq!(opened.version.citations, expected_citations());
    assert_eq!(opened.version.sources, expected_sources());
    assert!(!opened.version.written_without_search);
}

#[tokio::test]
async fn citation_events_arrive_while_the_section_streams_and_before_it_finishes() {
    let harness = Harness::new();
    harness.provider.script(grounded());

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    assert_eq!(
        citation_events(&generation),
        [
            (citation(0, 0, 45, 1), source(1, MANTLE_URL, MANTLE_TITLE)),
            (citation(0, 56, 85, 2), source(2, PLATES_URL, PLATES_TITLE)),
            (citation(2, 1, 20, 1), source(1, MANTLE_URL, MANTLE_TITLE)),
        ]
    );
    let first_citation = position(&generation, |event| {
        matches!(event, GenerationEvent::CitationAdded { .. })
    });
    let last_citation = generation
        .iter()
        .rposition(|event| matches!(event, GenerationEvent::CitationAdded { .. }))
        .unwrap();
    let figure_completed = position(
        &generation,
        |event| matches!(event, GenerationEvent::BlockCompleted { block } if block.index == 1),
    );
    let finished = position(&generation, |event| {
        matches!(event, GenerationEvent::Finished { .. })
    });
    assert!(
        first_citation < figure_completed,
        "the first citation arrives while the section streams: {generation:?}"
    );
    assert!(last_citation < finished, "{generation:?}");
    assert_eq!(
        generation.last(),
        Some(&GenerationEvent::Finished {
            outcome: GenerationOutcome::Completed
        })
    );
}

#[tokio::test]
async fn a_citation_inside_a_figure_attaches_to_the_end_of_the_prose_before_it() {
    let harness = Harness::new();
    let mut events = deltas(PROSE_0);
    events.extend(deltas(FIGURE_1));
    // Inside the figure's SVG.
    events.push(cite(PLATES_URL, PLATES_TITLE, 144, 150));
    events.extend(deltas(PROSE_2));
    events.extend(deltas(RECORD));
    events.push(completed());
    harness.provider.script(Script::Events(events));

    harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    let stored = harness
        .store
        .current_version(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("a version is stored");
    assert_eq!(stored.citations, [citation(0, 87, 87, 1)]);
    assert_eq!(stored.sources, [source(1, PLATES_URL, PLATES_TITLE)]);
}

// -- refusal --------------------------------------------------------------------

#[tokio::test]
async fn a_search_refused_before_the_stream_writes_the_section_again_without_search_for_the_session()
 {
    let harness = Harness::new();
    harness.provider.script(Script::Refused(search_refused()));
    harness.provider.script(plain());
    harness.provider.script(plain());

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.searches(), [true, false]);
    let stored = harness
        .store
        .current_version(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("a version is stored");
    assert!(stored.written_without_search);
    assert_eq!(stored.sources, []);
    assert_eq!(stored.citations, []);
    assert!(opened.version.written_without_search);
    assert_eq!(opened.version.sources, []);
    let generation = generation_events(&harness.events.take(), WHAT_DRIVES_PLATES);
    assert_eq!(
        generation.last(),
        Some(&GenerationEvent::Finished {
            outcome: GenerationOutcome::Completed
        })
    );

    // The refusal holds for the session: one request, without search.
    let next = harness.open(KINDS_OF_BOUNDARY).await.expect("written");
    assert_eq!(harness.searches(), [true, false, false]);
    assert!(next.version.written_without_search);
}

#[tokio::test]
async fn a_search_refused_during_the_stream_writes_the_section_again_without_search() {
    let harness = Harness::new();
    let mut refused = vec![Ok(ProviderEvent::Searching)];
    refused.extend(deltas(PROSE_0));
    refused.push(cite(MANTLE_URL, MANTLE_TITLE, 0, 45));
    refused.push(Err(search_refused()));
    harness.provider.script(Script::Events(refused));
    harness.provider.script(plain());
    harness.provider.script(plain());

    let opened = harness.open(WHAT_DRIVES_PLATES).await.expect("written");

    assert_eq!(harness.searches(), [true, false]);
    let stored = harness
        .store
        .current_version(WHAT_DRIVES_PLATES)
        .unwrap()
        .expect("a version is stored");
    assert!(stored.written_without_search);
    assert_eq!(stored.sources, []);
    assert_eq!(stored.citations, []);
    assert_eq!(prose(&stored.blocks, 0), PLAIN);
    assert!(opened.version.written_without_search);

    harness.open(KINDS_OF_BOUNDARY).await.expect("written");
    assert_eq!(harness.searches(), [true, false, false]);
}

#[tokio::test]
async fn a_plan_limit_before_the_stream_is_not_mistaken_for_a_refusal() {
    let harness = Harness::new();
    harness.provider.script(Script::Refused(plan_limit()));
    harness.provider.script(plain());

    let failed = harness.open(WHAT_DRIVES_PLATES).await;

    assert_eq!(failed, Err(CoreError::PlanLimit { resets_at: None }));
    assert_eq!(harness.searches(), [true]);
    assert_eq!(
        harness.store.current_version(WHAT_DRIVES_PLATES).unwrap(),
        None
    );

    // Search stays on for the next section.
    let next = harness.open(KINDS_OF_BOUNDARY).await.expect("written");
    assert_eq!(harness.searches(), [true, true]);
    assert!(!next.version.written_without_search);
}

#[tokio::test]
async fn a_plan_limit_during_the_stream_is_not_mistaken_for_a_refusal() {
    let harness = Harness::new();
    let mut limited = deltas(PROSE_0);
    limited.push(Err(plan_limit()));
    harness.provider.script(Script::Events(limited));
    harness.provider.script(plain());

    let failed = harness.open(WHAT_DRIVES_PLATES).await;

    assert_eq!(failed, Err(CoreError::PlanLimit { resets_at: None }));
    assert_eq!(harness.searches(), [true]);
    assert_eq!(
        harness.store.current_version(WHAT_DRIVES_PLATES).unwrap(),
        None
    );

    let next = harness.open(KINDS_OF_BOUNDARY).await.expect("written");
    assert_eq!(harness.searches(), [true, true]);
    assert!(!next.version.written_without_search);
}
