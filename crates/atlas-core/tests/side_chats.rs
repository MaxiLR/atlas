//! Side chats through the core's request handler: a chat anchored to a
//! passage or to the course alone, the reply request and what it carries, the
//! reply streamed with web search, stored with its citations and stripped of
//! its follow-up block, a stopped reply kept and marked, the list of chats,
//! and the isolation of everything said in a chat from the course memory.
//!
//! The sample course is laid down through the store's repository API on the
//! same database file in a temp dir. The provider answers each side-chat
//! request with a scripted stream of provider events and records every
//! request. Offsets and expected texts are written out by hand.

mod support;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionOrigin};
use atlas_core::protocol::{
    Anchor, Block, BlockContent, ChatMessage, ChatRole, Citation, CoreError, CoreEvent,
    CoreRequest, CoreResponse, GenerationEvent, GenerationOutcome, GenerationTarget, MessageState,
    Model, SideChat, SideChatSummary, Source, VersionStyle,
};
use atlas_core::provider::{
    ModelProvider, ProviderCitation, ProviderError, ProviderEvent, ProviderRequest, ProviderStream,
    Task,
};
use atlas_core::store::{NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, RecordingEvents};
use futures::StreamExt;
use futures::stream;
use support::sample_course::{
    KINDS_OF_BOUNDARY, LAYERS, LAYERS_V1, LITHOSPHERE, SampleCourse, WHAT_DRIVES_PLATES,
    sample_course,
};
use tempfile::TempDir;
use tokio::sync::Notify;
use uuid::Uuid;

const MODEL: &str = "test-model";

/// The two paragraphs of the current version of "Layers of the Earth".
const LAYERS_FIRST: &str =
    "Earth is built in layers: a thin crust, a thick mantle and a metal core.";
const LAYERS_SECOND: &str =
    "The mantle is solid rock that flows over millions of years, LAYERS-TEXT-5c2a.";
/// Quoted from the second paragraph.
const QUOTE: &str = "solid rock that flows";

/// A reply of 72 characters: "Plates move a few centimetres a year" is
/// 0..36 and "Hot spots leave chains of islands" is 38..71.
const REPLY: &str = "Plates move a few centimetres a year. Hot spots leave chains of islands.";
const SPEED_URL: &str = "https://example.org/plate-speed";
const SPEED_TITLE: &str = "How fast plates move";
const HOTSPOT_URL: &str = "https://example.org/hot-spots";
const HOTSPOT_TITLE: &str = "Hot spots";

const SENTINEL: &str = "SIDE-CHAT-SENTINEL-9f3b";

// -- the scripted provider -------------------------------------------------------

/// What one request is answered with.
enum Script {
    /// These events, in order.
    Events(Vec<Result<ProviderEvent, ProviderError>>),
    /// The first text, then a hold until released, then the rest and
    /// completed.
    Held(String, String),
    /// The call fails before any event.
    Refused(ProviderError),
}

/// Answers each task with its scripts in order and records every request.
/// A task without a script left fails.
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

    fn reply(&self, script: Script) {
        self.script(Task::SideChatReply, script);
    }

    fn requests_for(&self, task: Task) -> Vec<ProviderRequest> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.task == task)
            .cloned()
            .collect()
    }

    fn chat_requests(&self) -> Vec<ProviderRequest> {
        self.requests_for(Task::SideChatReply)
    }
}

/// `text` as deltas of at most five characters.
fn deltas(text: &str) -> Vec<Result<ProviderEvent, ProviderError>> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(5)
        .map(|chars| Ok(ProviderEvent::TextDelta(chars.iter().collect())))
        .collect()
}

fn completed() -> Result<ProviderEvent, ProviderError> {
    Ok(ProviderEvent::Completed)
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
        Ok(match script {
            Script::Events(events) => stream::iter(events).boxed(),
            Script::Refused(error) => return Err(error),
            Script::Held(first, rest) => {
                let (held, release) = (self.held.clone(), self.release.clone());
                let hold = stream::once(async move {
                    held.notify_one();
                    release.notified().await;
                    stream::iter(deltas(&rest))
                })
                .flatten();
                stream::iter(deltas(&first))
                    .chain(hold)
                    .chain(stream::iter([completed()]))
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

/// A reply of `text`, completed.
fn answer(text: &str) -> Script {
    let mut events = deltas(text);
    events.push(completed());
    Script::Events(events)
}

/// The reserved closing block offering `prompts`.
fn follow_up_block(prompts: &[&str]) -> String {
    let mut block = "\n\n```atlas-follow-ups\n".to_owned();
    for prompt in prompts {
        block.push_str(prompt);
        block.push('\n');
    }
    block.push_str("```\n");
    block
}

fn cite(url: &str, title: &str, start: u32, end: u32) -> Result<ProviderEvent, ProviderError> {
    Ok(ProviderEvent::Citation(ProviderCitation {
        url: url.to_owned(),
        title: title.to_owned(),
        start,
        end,
    }))
}

// -- the harness -----------------------------------------------------------------

struct Harness {
    _dir: TempDir,
    ports: FakePorts,
    provider: Arc<ScriptedProvider>,
    events: Arc<RecordingEvents>,
    core: Core,
    store: Store,
    /// The database file, for reading what is left under the store.
    path: PathBuf,
    course: Uuid,
    sample: SampleCourse,
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
        let sample = sample_course();
        let course = store_course(&store, &sample);
        events.take();
        Self {
            _dir: dir,
            ports,
            provider,
            events,
            core,
            store,
            path,
            course,
            sample,
        }
    }

    async fn delete(&self, chat: Uuid) -> Result<CoreResponse, CoreError> {
        self.core
            .handle(CoreRequest::DeleteSideChat { chat_id: chat })
            .await
    }

    async fn side_chat_count(&self) -> u32 {
        match self
            .core
            .handle(CoreRequest::OpenCourse {
                course_id: self.course,
            })
            .await
        {
            Ok(CoreResponse::Course { course }) => course.side_chat_count,
            other => panic!("expected the course, got {other:?}"),
        }
    }

    /// The chat's stored quote and deletion time, read under the store.
    fn raw_chat(&self, chat: Uuid) -> (Option<String>, Option<String>) {
        rusqlite::Connection::open(&self.path)
            .unwrap()
            .query_row(
                "SELECT quote, deleted_at FROM side_chats WHERE id = ?1",
                [chat.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    }

    /// Every stored message of the chat, deleted or not, read under the
    /// store: text, citations, sources and deletion time.
    fn raw_messages(&self, chat: Uuid) -> Vec<RawMessage> {
        let connection = rusqlite::Connection::open(&self.path).unwrap();
        let mut statement = connection
            .prepare(
                "SELECT text, citations, sources, deleted_at FROM side_chat_messages
                 WHERE chat_id = ?1 ORDER BY rowid",
            )
            .unwrap();
        statement
            .query_map([chat.to_string()], |row| {
                Ok(RawMessage {
                    text: row.get(0)?,
                    citations: row.get(1)?,
                    sources: row.get(2)?,
                    deleted_at: row.get(3)?,
                })
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    async fn start(&self, anchor: Option<Anchor>) -> SideChat {
        match self
            .core
            .handle(CoreRequest::StartSideChat {
                course_id: self.course,
                anchor,
            })
            .await
        {
            Ok(CoreResponse::SideChat { chat }) => chat,
            other => panic!("expected a side chat, got {other:?}"),
        }
    }

    async fn send(&self, chat: Uuid, text: &str) -> Result<ChatMessage, CoreError> {
        self.core
            .handle(CoreRequest::SendSideChatMessage {
                chat_id: chat,
                text: text.to_owned(),
            })
            .await
            .map(|response| match response {
                CoreResponse::SideChatReply { chat_id, message } => {
                    assert_eq!(chat_id, chat);
                    message
                }
                other => panic!("expected a reply, got {other:?}"),
            })
    }

    async fn stop(&self, chat: Uuid) {
        assert_eq!(
            self.core
                .handle(CoreRequest::StopSideChatReply { chat_id: chat })
                .await,
            Ok(CoreResponse::Done)
        );
    }

    async fn open(&self, chat: Uuid) -> SideChat {
        match self
            .core
            .handle(CoreRequest::OpenSideChat { chat_id: chat })
            .await
        {
            Ok(CoreResponse::SideChat { chat }) => chat,
            other => panic!("expected a side chat, got {other:?}"),
        }
    }

    async fn list(&self) -> Vec<SideChatSummary> {
        match self
            .core
            .handle(CoreRequest::ListSideChats {
                course_id: self.course,
            })
            .await
        {
            Ok(CoreResponse::SideChats { course_id, chats }) => {
                assert_eq!(course_id, self.course);
                chats
            }
            other => panic!("expected the side chats, got {other:?}"),
        }
    }

    /// Everything a side chat must leave as it is: the profile, the map, the
    /// conventions, the index, and every version of every section with its
    /// record.
    fn memory(&self) -> String {
        let mut sections = Vec::new();
        for chapter in &self.sample.index.chapters {
            let mut stack: Vec<_> = chapter.sections.iter().collect();
            while let Some(section) = stack.pop() {
                sections.push(section.id);
                stack.extend(section.children.iter());
            }
        }
        sections.sort();
        let versions: Vec<_> = sections
            .iter()
            .map(|section| {
                (
                    section,
                    self.store.versions(*section).unwrap(),
                    self.store.current_record(*section).unwrap(),
                )
            })
            .collect();
        format!(
            "{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
            self.store.profile(self.course).unwrap(),
            self.store.course_map(self.course).unwrap(),
            self.store.conventions(self.course).unwrap(),
            self.store.index_tree(self.course).unwrap(),
            versions,
        )
    }
}

/// Lays the course down: ladder, profile, index with its map and the plan's
/// conventions, then a version for every record with the conventions it
/// fixed. The version of "Layers of the Earth" holds two paragraphs and a
/// diagram; the others one paragraph naming their version.
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
        let blocks = if record.section == LAYERS {
            vec![prose(0, LAYERS_FIRST), prose(1, LAYERS_SECOND)]
        } else {
            vec![prose(
                0,
                &format!("The text of version {}.", record.version),
            )]
        };
        store
            .insert_version(
                &NewSectionVersion {
                    id: record.version,
                    node: record.section,
                    style: VersionStyle::Original,
                    blocks,
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

fn prose(index: u32, text: &str) -> Block {
    Block {
        index,
        content: BlockContent::Prose {
            text: text.to_owned(),
        },
    }
}

fn anchor() -> Anchor {
    Anchor {
        version_id: LAYERS_V1,
        block: 1,
        quote: QUOTE.to_owned(),
    }
}

/// The generation events of the replies in `chat`, in order.
fn reply_events(events: &[CoreEvent], chat: Uuid) -> Vec<GenerationEvent> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::Generation {
                target: GenerationTarget::SideChat { chat_id, .. },
                event,
                ..
            } if *chat_id == chat => Some(event.clone()),
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

/// The text the deltas carried.
fn streamed(events: &[GenerationEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn follow_up_events(events: &[CoreEvent], chat: Uuid) -> Vec<Vec<String>> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::FollowUpPrompts { chat_id, prompts } if *chat_id == chat => {
                Some(prompts.clone())
            }
            _ => None,
        })
        .collect()
}

fn texts(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
}

// -- the request -----------------------------------------------------------------

#[tokio::test]
async fn the_request_for_an_anchored_chat_carries_the_passage_its_section_the_titles_the_profile_and_the_history()
 {
    let harness = Harness::new();
    let chat = harness.start(Some(anchor())).await;
    assert_eq!(chat.anchor, Some(anchor()));
    assert_eq!(chat.section_title.as_deref(), Some("Layers of the Earth"));
    assert!(chat.messages.is_empty());
    harness
        .provider
        .reply(answer("It deforms slowly, like ice in a glacier."));
    harness.provider.reply(answer("Over millions of years."));

    harness
        .send(chat.id, "How can solid rock flow?")
        .await
        .expect("replied");
    harness
        .send(chat.id, "How long does it take?")
        .await
        .expect("replied");

    let requests = harness.provider.chat_requests();
    assert_eq!(requests.len(), 2);
    let request = &requests[1];
    assert!(request.web_search, "search is on");
    assert_eq!(request.model.as_deref(), Some(MODEL));
    assert!(
        request.instructions.contains("atlas-follow-ups"),
        "the prompt names the follow-up block: {}",
        request.instructions
    );
    let input = &request.input;
    for expected in [QUOTE, LAYERS_FIRST, LAYERS_SECOND] {
        assert!(input.contains(expected), "{expected:?} in {input}");
    }
    for title in [
        "Earth's structure",
        "Layers of the Earth",
        "Lithosphere and asthenosphere",
        "Plates in motion",
        "What a plate is",
        "How plates are mapped",
        "What drives the plates",
        "Where plates meet",
        "Kinds of boundary",
    ] {
        assert!(input.contains(title), "title {title:?} in {input}");
    }
    for profile in [
        "working knowledge",
        "Mantle convection",
        "The mantle is a sea of liquid rock.",
    ] {
        assert!(input.contains(profile), "profile {profile:?} in {input}");
    }
    let turns = [
        "How can solid rock flow?",
        "It deforms slowly, like ice in a glacier.",
        "How long does it take?",
    ];
    let at: Vec<usize> = turns
        .iter()
        .map(|turn| {
            input
                .find(turn)
                .unwrap_or_else(|| panic!("{turn:?} in {input}"))
        })
        .collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{input}");
    // The course memory stays out (ADR-0013).
    assert!(!input.contains("The text of version"), "{input}");
}

#[tokio::test]
async fn a_chat_started_without_a_selection_works_and_sends_no_section_text() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    assert_eq!(chat.anchor, None);
    assert_eq!(chat.section_title, None);
    harness
        .provider
        .reply(answer("Mostly by the pull of sinking slabs."));

    let reply = harness
        .send(chat.id, "What moves the plates?")
        .await
        .expect("replied");

    assert_eq!(reply.role, ChatRole::Agent);
    assert_eq!(reply.state, MessageState::Complete);
    assert_eq!(reply.text, "Mostly by the pull of sinking slabs.");
    let requests = harness.provider.chat_requests();
    assert_eq!(requests.len(), 1);
    let input = &requests[0].input;
    assert!(input.contains("What moves the plates?"), "{input}");
    assert!(input.contains("What drives the plates"), "{input}");
    for text in [LAYERS_FIRST, LAYERS_SECOND, "The text of version"] {
        assert!(!input.contains(text), "{text:?} in {input}");
    }
    let thread = harness.open(chat.id).await;
    let roles: Vec<_> = thread
        .messages
        .iter()
        .map(|message| (message.role, message.text.as_str()))
        .collect();
    assert_eq!(
        roles,
        [
            (ChatRole::Candidate, "What moves the plates?"),
            (ChatRole::Agent, "Mostly by the pull of sinking slabs."),
        ]
    );
}

// -- the reply -------------------------------------------------------------------

#[tokio::test]
async fn a_reply_is_stored_with_its_two_citations_and_its_follow_up_block_becomes_three_prompts() {
    assert_eq!(&REPLY[..36], "Plates move a few centimetres a year");
    assert_eq!(&REPLY[38..71], "Hot spots leave chains of islands");
    let harness = Harness::new();
    let chat = harness.start(None).await;
    let prompts = [
        "Why do hot spots stay put?",
        "How is plate speed measured?",
        "Do all plates move at the same speed?",
    ];
    let mut events = vec![Ok(ProviderEvent::Searching)];
    events.extend(deltas(REPLY));
    events.push(cite(SPEED_URL, SPEED_TITLE, 0, 36));
    events.push(cite(HOTSPOT_URL, HOTSPOT_TITLE, 38, 71));
    events.extend(deltas(&follow_up_block(&prompts)));
    events.push(completed());
    harness.provider.reply(Script::Events(events));

    let reply = harness
        .send(chat.id, "How fast do plates move?")
        .await
        .expect("replied");

    let citations = vec![
        Citation {
            block: 0,
            start: 0,
            end: 36,
            source: 1,
        },
        Citation {
            block: 0,
            start: 38,
            end: 71,
            source: 2,
        },
    ];
    let sources = vec![
        Source {
            number: 1,
            url: SPEED_URL.to_owned(),
            title: SPEED_TITLE.to_owned(),
        },
        Source {
            number: 2,
            url: HOTSPOT_URL.to_owned(),
            title: HOTSPOT_TITLE.to_owned(),
        },
    ];
    assert_eq!(reply.text, REPLY);
    assert_eq!(reply.state, MessageState::Complete);
    assert_eq!(reply.citations, citations);
    assert_eq!(reply.sources, sources);

    let thread = harness.open(chat.id).await;
    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[1], reply, "stored as answered");
    assert_eq!(thread.follow_ups, texts(&prompts));

    let events = harness.events.take();
    assert_eq!(follow_up_events(&events, chat.id), [texts(&prompts)]);
    let generation = reply_events(&events, chat.id);
    assert_eq!(outcome(&generation), Some(GenerationOutcome::Completed));
    let shown = streamed(&generation);
    assert!(!shown.contains("atlas-follow-ups"), "{shown:?}");
    assert!(!shown.contains(prompts[0]), "{shown:?}");
    let cited: Vec<_> = generation
        .iter()
        .filter_map(|event| match event {
            GenerationEvent::CitationAdded { citation, .. } => Some(citation.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(cited, citations);
}

#[tokio::test]
async fn five_proposed_follow_up_prompts_yield_the_first_three() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    let prompts = [
        "What is a hot spot?",
        "Where is Hawaii's hot spot?",
        "Can a hot spot move?",
        "What is a mantle plume?",
        "How old are the islands?",
    ];
    let text = format!("Islands form over a hot spot.{}", follow_up_block(&prompts));
    harness.provider.reply(answer(&text));

    let reply = harness
        .send(chat.id, "Why are islands in a line?")
        .await
        .expect("replied");

    assert_eq!(reply.text, "Islands form over a hot spot.");
    let events = harness.events.take();
    assert_eq!(follow_up_events(&events, chat.id), [texts(&prompts[..3])]);
    assert_eq!(harness.open(chat.id).await.follow_ups, texts(&prompts[..3]));
}

#[tokio::test]
async fn stop_keeps_the_partial_reply_marked_stopped_and_the_next_context_carries_it_as_such() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    let first = "Convection carries heat upward. It also";
    harness.provider.reply(Script::Held(
        first.to_owned(),
        " drags the plates along.\n".to_owned(),
    ));

    let (sent, ()) = tokio::join!(harness.send(chat.id, "What does convection do?"), async {
        harness.provider.held.notified().await;
        harness.stop(chat.id).await;
    });

    let stopped = sent.expect("the partial reply is kept");
    assert_eq!(stopped.role, ChatRole::Agent);
    assert_eq!(stopped.state, MessageState::Stopped);
    assert_eq!(stopped.text, first);
    let generation = reply_events(&harness.events.take(), chat.id);
    assert_eq!(outcome(&generation), Some(GenerationOutcome::Stopped));
    let thread = harness.open(chat.id).await;
    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[1], stopped);

    harness.provider.reply(answer("It drags the plates along."));
    harness
        .send(chat.id, "Please finish.")
        .await
        .expect("replied");

    let requests = harness.provider.chat_requests();
    assert_eq!(requests.len(), 2);
    let input = &requests[1].input;
    assert!(
        input.contains(&format!(
            "## Atlas, reply stopped before it finished\n{first}"
        )),
        "{input}"
    );
}

#[tokio::test]
async fn a_failed_reply_stores_nothing_and_reports_its_error() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    harness
        .provider
        .reply(Script::Refused(ProviderError::PlanLimit {
            resets_at: None,
            request_id: Some("req_limit".to_owned()),
        }));

    let sent = harness.send(chat.id, "Why do plates move?").await;

    assert_eq!(sent, Err(CoreError::PlanLimit { resets_at: None }));
    let generation = reply_events(&harness.events.take(), chat.id);
    assert_eq!(
        outcome(&generation),
        Some(GenerationOutcome::Failed {
            error: CoreError::PlanLimit { resets_at: None }
        })
    );
    let thread = harness.open(chat.id).await;
    let roles: Vec<_> = thread.messages.iter().map(|message| message.role).collect();
    assert_eq!(roles, [ChatRole::Candidate], "no reply stored");
}

#[tokio::test]
async fn a_refused_search_gets_the_reply_written_once_more_without_it() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    harness
        .provider
        .reply(Script::Refused(ProviderError::Unsupported {
            param: Some("tools".to_owned()),
            request_id: Some("req_search".to_owned()),
        }));
    harness.provider.reply(answer("Slowly."));
    harness.provider.reply(answer("Very slowly."));

    let reply = harness.send(chat.id, "How fast?").await.expect("replied");
    assert_eq!(reply.text, "Slowly.");
    harness.send(chat.id, "Really?").await.expect("replied");

    let searches: Vec<_> = harness
        .provider
        .chat_requests()
        .iter()
        .map(|request| request.web_search)
        .collect();
    assert_eq!(searches, [true, false, false]);
}

// -- the list --------------------------------------------------------------------

#[tokio::test]
async fn chats_are_listed_newest_first_with_their_passage_or_first_question() {
    let harness = Harness::new();
    let anchored = harness.start(Some(anchor())).await;
    harness.ports.clock.advance(Duration::from_secs(60));
    let unanchored = harness.start(None).await;
    harness.provider.reply(answer("Where they pull apart."));
    harness.provider.reply(answer("At ridges."));
    harness
        .send(unanchored.id, "Where do new plates form?")
        .await
        .expect("replied");
    harness
        .send(unanchored.id, "And where exactly?")
        .await
        .expect("replied");

    let listed: Vec<_> = harness
        .list()
        .await
        .into_iter()
        .map(|chat| (chat.id, chat.anchor, chat.section_title, chat.opening))
        .collect();

    assert_eq!(
        listed,
        [
            (
                unanchored.id,
                None,
                None,
                "Where do new plates form?".to_owned()
            ),
            (
                anchored.id,
                Some(anchor()),
                Some("Layers of the Earth".to_owned()),
                QUOTE.to_owned()
            ),
        ]
    );
}

// -- isolation -------------------------------------------------------------------

#[tokio::test]
async fn nothing_said_in_a_chat_reaches_the_course_memory_or_a_later_section_request() {
    let harness = Harness::new();
    let before = harness.memory();
    let anchored = harness.start(Some(anchor())).await;
    let unanchored = harness.start(None).await;
    harness
        .provider
        .reply(answer(&format!("Noted: {SENTINEL}. The mantle flows.")));
    harness
        .provider
        .reply(answer(&format!("Noted again: {SENTINEL}.")));

    harness
        .send(anchored.id, &format!("Remember {SENTINEL} for later."))
        .await
        .expect("replied");
    harness
        .send(unanchored.id, &format!("Use {SENTINEL} in every section."))
        .await
        .expect("replied");

    assert_eq!(harness.memory(), before, "the course memory is unchanged");
    // A section request built afterwards; it has no script, so it fails
    // after being recorded and stores nothing.
    for section in [WHAT_DRIVES_PLATES, KINDS_OF_BOUNDARY] {
        let opened = harness
            .core
            .handle(CoreRequest::OpenSection {
                course_id: harness.course,
                section_id: section,
            })
            .await;
        assert!(opened.is_err(), "{opened:?}");
    }
    let sections = harness.provider.requests_for(Task::WriteSection);
    assert_eq!(sections.len(), 2);
    for request in &sections {
        assert!(!request.input.contains(SENTINEL), "{}", request.input);
        assert!(!request.instructions.contains(SENTINEL));
    }
    assert_eq!(harness.memory(), before);
    assert!(harness.store.current_record(LITHOSPHERE).unwrap().is_some());
}

// -- deleting a chat --------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
struct RawMessage {
    text: String,
    citations: String,
    sources: String,
    deleted_at: Option<String>,
}

/// The chats a side-chats-changed event names.
fn changed_chats(events: &[CoreEvent]) -> Vec<Uuid> {
    events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::SideChatsChanged { chat, .. } => Some(chat.id),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_deleted_chat_leaves_the_list_and_the_count_and_its_passage_and_messages_are_erased() {
    let harness = Harness::new();
    let doomed = harness.start(Some(anchor())).await;
    harness.provider.reply(Script::Events(vec![
        Ok(ProviderEvent::TextDelta(format!("{REPLY} {SENTINEL}"))),
        cite(SPEED_URL, SPEED_TITLE, 0, 36),
        completed(),
    ]));
    harness
        .send(doomed.id, "How fast do plates move?")
        .await
        .expect("replied");
    let kept = harness.start(None).await;
    harness.provider.reply(answer("Heat from the core."));
    harness
        .send(kept.id, "What drives the plates?")
        .await
        .expect("replied");
    let kept_thread = harness.open(kept.id).await;
    let kept_listed = harness.list().await;
    assert_eq!(kept_listed.len(), 2);
    assert_eq!(harness.side_chat_count().await, 2);
    let kept_messages = harness.raw_messages(kept.id);
    harness.events.take();

    assert_eq!(harness.delete(doomed.id).await, Ok(CoreResponse::Done));

    assert_eq!(changed_chats(&harness.events.take()), Vec::<Uuid>::new());
    let listed = harness.list().await;
    assert_eq!(
        listed,
        kept_listed
            .into_iter()
            .filter(|chat| chat.id == kept.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(harness.side_chat_count().await, 1);
    let opened = harness
        .core
        .handle(CoreRequest::OpenSideChat { chat_id: doomed.id })
        .await;
    assert!(
        matches!(opened, Err(CoreError::NotFound { .. })),
        "expected not found, got {opened:?}"
    );

    // The chat and its messages stay as tombstones with their content
    // erased; the other chat is as it was.
    let (quote, deleted_at) = harness.raw_chat(doomed.id);
    assert_eq!(quote.as_deref(), Some(""), "the passage is erased");
    assert!(deleted_at.is_some(), "the chat is tombstoned");
    let messages = harness.raw_messages(doomed.id);
    assert_eq!(messages.len(), 2, "both messages are kept as tombstones");
    for message in &messages {
        assert!(message.deleted_at.is_some(), "tombstoned: {message:?}");
        assert_eq!(
            (
                message.text.as_str(),
                message.citations.as_str(),
                message.sources.as_str()
            ),
            ("", "", ""),
            "erased: {message:?}"
        );
    }
    assert_eq!(harness.raw_messages(kept.id), kept_messages);
    assert_eq!(harness.open(kept.id).await, kept_thread);
}

#[tokio::test]
async fn deleting_a_chat_while_its_reply_streams_stops_it_and_stores_nothing_after() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    let other = harness.start(None).await;
    harness.provider.reply(Script::Held(
        "Convection carries heat upward. It also".to_owned(),
        format!(" drags the plates along. {SENTINEL}\n"),
    ));
    harness.events.take();

    let (sent, after_done) =
        tokio::join!(harness.send(chat.id, "What does convection do?"), async {
            harness.provider.held.notified().await;
            let deleted = harness.delete(chat.id).await;
            // What is stored and announced from the moment Done answers.
            let at_done = harness.raw_messages(chat.id);
            harness.events.take();
            // Lets a reply that was not stopped run to its end.
            harness.provider.release.notify_one();
            (deleted, at_done)
        });
    let (deleted, at_done) = after_done;

    assert_eq!(deleted, Ok(CoreResponse::Done));
    // The pending send settled; whatever it answered, nothing of the chat
    // holds text, then or now.
    let _ = sent;
    for messages in [at_done, harness.raw_messages(chat.id)] {
        assert!(
            messages
                .iter()
                .all(|message| message.text.is_empty() && message.deleted_at.is_some()),
            "nothing of the chat is kept: {messages:?}"
        );
    }
    let events = harness.events.take();
    assert!(
        !changed_chats(&events).contains(&chat.id),
        "no side-chats-changed for the chat after Done: {events:?}"
    );

    // The other chat still replies, and the deleted one cannot.
    harness.provider.reply(answer("It drags the plates along."));
    harness
        .send(other.id, "And what does it drag?")
        .await
        .expect("the other chat replies");
    let resent = harness.send(chat.id, "Hello?").await;
    assert!(
        matches!(resent, Err(CoreError::NotFound { .. })),
        "expected not found, got {resent:?}"
    );
}

#[tokio::test]
async fn deleting_a_chat_while_its_reply_streams_has_stopped_and_settled_it_when_done_answers() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    harness.provider.reply(Script::Held(
        "Convection carries heat upward. It also".to_owned(),
        format!(" drags the plates along. {SENTINEL}\n"),
    ));
    harness.events.take();
    let send_settled = std::cell::Cell::new(false);

    let (sent, (deleted, settled_at_done, events_at_done)) = tokio::join!(
        async {
            let sent = harness.send(chat.id, "What does convection do?").await;
            send_settled.set(true);
            sent
        },
        async {
            harness.provider.held.notified().await;
            // The held stream is never released before Done: only a stop
            // can end the reply.
            let deleted = harness.delete(chat.id).await;
            let settled = send_settled.get();
            let events = harness.events.take();
            // Lets a reply that was not stopped run to its end, so that a
            // failing run ends rather than hangs.
            harness.provider.release.notify_one();
            (deleted, settled, events)
        }
    );

    assert_eq!(deleted, Ok(CoreResponse::Done));
    assert!(
        settled_at_done,
        "the pending send had settled when Done answered"
    );
    let generation = reply_events(&events_at_done, chat.id);
    let ended = outcome(&generation);
    assert!(
        ended.is_some() && ended != Some(GenerationOutcome::Completed),
        "the reply was stopped before Done answered: {generation:?}"
    );
    assert!(
        !streamed(&generation).contains(SENTINEL),
        "nothing after the hold was streamed: {generation:?}"
    );
    assert!(sent.is_err(), "a reply for a deleted chat is not kept");
}

#[tokio::test]
async fn deleting_an_unknown_or_already_deleted_chat_is_not_found() {
    let harness = Harness::new();
    let chat = harness.start(None).await;
    assert_eq!(harness.delete(chat.id).await, Ok(CoreResponse::Done));

    for id in [chat.id, Uuid::new_v4()] {
        let answer = harness.delete(id).await;
        assert!(
            matches!(answer, Err(CoreError::NotFound { .. })),
            "expected not found for {id}, got {answer:?}"
        );
    }
}
