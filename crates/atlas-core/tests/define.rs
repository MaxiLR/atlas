//! Define and the course glossary through the core's request handler, with a
//! scripted fake provider, on a database in a temp dir.
//!
//! The course (ladder, profile, index, versions and a side chat) is laid down
//! through the store's repository API on the same file.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::Core;
use atlas_core::protocol::{
    Anchor, Block, BlockContent, ChatRole, CoreError, CoreEvent, CoreRequest, CoreResponse,
    GenerationEvent, GenerationOutcome, GenerationTarget, GlossaryEntry, MessageState,
    VersionStyle,
};
use atlas_core::provider::{FakeProvider, FakeScript, ProviderError, ProviderRequest, Task};
use atlas_core::store::{NewChatMessage, NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, RecordingEvents};
use support::sample_course::{LAYERS, LAYERS_V1, LITHOSPHERE, LITHOSPHERE_V1, sample_course};
use tempfile::TempDir;
use uuid::Uuid;

const TERM: &str = "Mohorovičić discontinuity";
const SENTENCE: &str =
    "Below the crust lies the Mohorovičić discontinuity, where seismic waves speed up.";
const LAYERS_OTHER_BLOCK: &str = "The core is mostly iron and nickel.";
const LITHOSPHERE_TEXT: &str = "The asthenosphere flows like warm wax beneath the rigid shell.";
const CHAT_QUESTION: &str = "Is the boundary a sharp line or a gradual zone?";
const CHAT_REPLY: &str = "It is sharp enough that seismographs pick it up as a jump.";

const DEFINITION: &str = "The Mohorovičić discontinuity is the boundary between the Earth's \
                          crust and the mantle beneath it. Seismic waves speed up as they \
                          cross it, which is how it was found.";
const SECOND_DEFINITION: &str = "The boundary where the crust ends and the mantle begins. \
                                 Seismic waves travel faster below it.";

/// The title of the section the term is defined in, as the sample index
/// names it.
const LAYERS_TITLE: &str = "Layers of the Earth";

struct Harness {
    _dir: TempDir,
    core: Core,
    provider: Arc<FakeProvider>,
    events: Arc<RecordingEvents>,
    /// The same database file, for laying the course down.
    store: Store,
    course: Uuid,
}

impl Harness {
    fn new(provider: FakeProvider) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(provider);
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();

        let sample = sample_course();
        let course = store.create_course(&sample.topic).unwrap().id;
        store.store_ladder(course, &sample.ladder).unwrap();
        store.save_profile(course, &sample.profile).unwrap();
        store
            .insert_index(course, &sample.index, &sample.map, &[])
            .unwrap();
        let harness = Self {
            _dir: dir,
            core,
            provider,
            events,
            store,
            course,
        };
        harness.insert_version(
            LAYERS,
            LAYERS_V1,
            vec![prose(0, SENTENCE), prose(1, LAYERS_OTHER_BLOCK)],
        );
        harness.insert_version(
            LITHOSPHERE,
            LITHOSPHERE_V1,
            vec![prose(0, LITHOSPHERE_TEXT)],
        );

        let chat = harness
            .store
            .start_side_chat(course, Some(&anchor(LAYERS_V1)))
            .unwrap();
        for (role, text) in [
            (ChatRole::Candidate, CHAT_QUESTION),
            (ChatRole::Agent, CHAT_REPLY),
        ] {
            harness
                .store
                .add_chat_message(
                    chat.id,
                    &NewChatMessage {
                        role,
                        text: text.to_owned(),
                        state: MessageState::Complete,
                        citations: Vec::new(),
                        sources: Vec::new(),
                    },
                )
                .unwrap();
        }
        harness.events.take();
        harness
    }

    fn insert_version(&self, section: Uuid, version: Uuid, blocks: Vec<Block>) {
        let sample = sample_course();
        let record = sample
            .records
            .iter()
            .find(|record| record.section == section)
            .map_or_else(
                || sample.records[0].record.clone(),
                |record| record.record.clone(),
            );
        self.store
            .insert_version(
                &NewSectionVersion {
                    id: version,
                    node: section,
                    style: VersionStyle::Original,
                    blocks,
                    checks: Vec::new(),
                    citations: Vec::new(),
                    sources: Vec::new(),
                    written_without_search: false,
                    record,
                    model: Some("gpt-5".to_owned()),
                    prompt: "write-section@1".to_owned(),
                },
                &[],
            )
            .unwrap();
    }

    async fn define(&self, anchor: Anchor) -> Result<GlossaryEntry, CoreError> {
        self.core
            .handle(CoreRequest::Define {
                anchor,
                sentence: SENTENCE.to_owned(),
            })
            .await
            .map(|response| match response {
                CoreResponse::Definition { entry } => entry,
                other => panic!("expected a definition, got {other:?}"),
            })
    }

    async fn glossary(&self) -> Vec<GlossaryEntry> {
        match self
            .core
            .handle(CoreRequest::ListGlossary {
                course_id: self.course,
            })
            .await
        {
            Ok(CoreResponse::Glossary { course_id, entries }) => {
                assert_eq!(course_id, self.course, "the answer names the course");
                entries
            }
            other => panic!("expected the glossary, got {other:?}"),
        }
    }

    fn requests(&self) -> Vec<ProviderRequest> {
        self.provider.requests()
    }
}

fn prose(index: u32, text: &str) -> Block {
    Block {
        index,
        content: BlockContent::Prose {
            text: text.to_owned(),
        },
    }
}

fn anchor(version: Uuid) -> Anchor {
    Anchor {
        version_id: version,
        block: 0,
        quote: TERM.to_owned(),
    }
}

fn provider(answers: impl IntoIterator<Item = Result<String, ProviderError>>) -> FakeProvider {
    FakeProvider::new(FakeScript::sample()).answer(Task::Define, answers)
}

/// The generation events of definitions, in order.
fn define_generation(events: &[CoreEvent]) -> Vec<(GenerationTarget, GenerationEvent)> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::Generation { target, event, .. }
                if matches!(target, GenerationTarget::Define { .. }) =>
            {
                Some((target.clone(), event.clone()))
            }
            _ => None,
        })
        .collect()
}

fn glossary_changes(events: &[CoreEvent]) -> Vec<(Uuid, GlossaryEntry)> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::GlossaryChanged { course_id, entry } => Some((*course_id, entry.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_request_carries_the_selection_its_sentence_and_the_profile_and_nothing_else() {
    let harness = Harness::new(provider([Ok(DEFINITION.to_owned())]));

    harness.define(anchor(LAYERS_V1)).await.expect("defined");

    let requests = harness.requests();
    assert_eq!(requests.len(), 1, "one request: {requests:?}");
    let request = &requests[0];
    assert_eq!(request.task, Task::Define);
    assert!(!request.web_search, "no web search");
    assert_eq!(request.schema, None, "plain text, no schema");
    assert!(
        request.instructions.contains("two to four sentences"),
        "the define prompt: {}",
        request.instructions
    );
    let input = &request.input;
    assert!(input.contains(TERM), "the selected text: {input}");
    assert!(input.contains(SENTENCE), "the sentence: {input}");
    // The sample profile: working knowledge, rungs 1-2 known, 3 shaky, the
    // rest unknown, and the misconception picked on rung 4.
    for profile in [
        "working knowledge",
        "Tectonic plates",
        "shaky",
        "Mantle convection",
        "The mantle is a sea of liquid rock.",
    ] {
        assert!(input.contains(profile), "the profile ({profile}): {input}");
    }
    for elsewhere in [
        LITHOSPHERE_TEXT,
        CHAT_QUESTION,
        CHAT_REPLY,
        "By strength, the crust and the top of the mantle form one rigid",
    ] {
        assert!(
            !input.contains(elsewhere),
            "nothing from side chats or other sections ({elsewhere}): {input}"
        );
    }
}

#[tokio::test]
async fn a_completed_definition_is_stored_with_its_anchor_listed_with_its_section_and_announced() {
    let harness = Harness::new(provider([Ok(DEFINITION.to_owned())]));

    let entry = harness.define(anchor(LAYERS_V1)).await.expect("defined");

    assert_eq!(entry.course_id, harness.course);
    assert_eq!(entry.section_id, LAYERS);
    assert_eq!(entry.section_title, LAYERS_TITLE);
    assert_eq!(entry.term, TERM);
    assert_eq!(entry.definition, DEFINITION);
    assert_eq!(entry.anchor, anchor(LAYERS_V1));
    assert_eq!(entry.wikipedia, None);

    assert_eq!(harness.glossary().await, vec![entry.clone()]);

    let events = harness.events.take();
    let generation = define_generation(&events);
    let target = GenerationTarget::Define {
        course_id: harness.course,
        anchor: anchor(LAYERS_V1),
    };
    assert!(
        generation.iter().all(|(seen, _)| *seen == target),
        "every step names the definition: {generation:?}"
    );
    let steps: Vec<_> = generation.into_iter().map(|(_, event)| event).collect();
    assert_eq!(steps.first(), Some(&GenerationEvent::Started), "{steps:?}");
    assert_eq!(
        steps.last(),
        Some(&GenerationEvent::Finished {
            outcome: GenerationOutcome::Completed
        }),
        "{steps:?}"
    );
    let streamed: String = steps
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(streamed, DEFINITION, "the text streams: {steps:?}");
    assert_eq!(
        glossary_changes(&events),
        vec![(harness.course, entry)],
        "the glossary-changed event carries the entry"
    );
}

#[tokio::test]
async fn defining_the_same_anchor_again_makes_no_request_and_returns_the_stored_entry() {
    let harness = Harness::new(provider([
        Ok(DEFINITION.to_owned()),
        Ok(SECOND_DEFINITION.to_owned()),
    ]));
    let first = harness.define(anchor(LAYERS_V1)).await.expect("defined");
    harness.events.take();

    let again = harness.define(anchor(LAYERS_V1)).await.expect("defined");

    assert_eq!(harness.requests().len(), 1, "no second request");
    assert_eq!(again, first, "the same entry");
    assert_eq!(again.definition, DEFINITION);
    let events = harness.events.take();
    assert_eq!(define_generation(&events), vec![], "nothing generated");
    assert_eq!(
        glossary_changes(&events),
        vec![],
        "the glossary is unchanged"
    );
    assert_eq!(harness.glossary().await, vec![first]);
}

#[tokio::test]
async fn a_failed_request_stores_nothing_and_reports_its_error() {
    let harness = Harness::new(provider([
        Err(ProviderError::Offline),
        Ok(DEFINITION.to_owned()),
    ]));

    let failed = harness.define(anchor(LAYERS_V1)).await;

    assert_eq!(failed, Err(CoreError::Offline));
    let events = harness.events.take();
    let steps: Vec<_> = define_generation(&events)
        .into_iter()
        .map(|(_, event)| event)
        .collect();
    assert_eq!(
        steps.last(),
        Some(&GenerationEvent::Finished {
            outcome: GenerationOutcome::Failed {
                error: CoreError::Offline
            }
        }),
        "the generation ends failed: {steps:?}"
    );
    assert_eq!(glossary_changes(&events), vec![], "no glossary change");
    assert_eq!(harness.glossary().await, vec![], "nothing stored");

    let entry = harness
        .define(anchor(LAYERS_V1))
        .await
        .expect("defined on the next try");
    assert_eq!(harness.requests().len(), 2, "the next try asks again");
    assert_eq!(entry.definition, DEFINITION);
}

#[tokio::test]
async fn an_entry_belongs_to_its_version_and_not_to_a_later_version_of_the_section() {
    let harness = Harness::new(provider([
        Ok(DEFINITION.to_owned()),
        Ok(SECOND_DEFINITION.to_owned()),
    ]));
    let first = harness.define(anchor(LAYERS_V1)).await.expect("defined");
    let later = Uuid::from_u128(0x1101_0002);
    harness.insert_version(
        LAYERS,
        later,
        vec![prose(0, SENTENCE), prose(1, LAYERS_OTHER_BLOCK)],
    );

    let on_later = harness.define(anchor(later)).await.expect("defined");

    assert_eq!(
        harness.requests().len(),
        2,
        "the later version is defined anew"
    );
    assert_ne!(on_later.id, first.id);
    assert_eq!(on_later.anchor, anchor(later));
    assert_eq!(on_later.definition, SECOND_DEFINITION);
    assert_eq!(on_later.section_title, LAYERS_TITLE);

    let again = harness.define(anchor(LAYERS_V1)).await.expect("defined");
    assert_eq!(again, first, "the first version keeps its own entry");
    assert_eq!(harness.requests().len(), 2, "with no request");
    assert_eq!(harness.glossary().await, vec![first, on_later]);
}
