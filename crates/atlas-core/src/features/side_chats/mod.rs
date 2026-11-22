//! Side chats beside the lesson: anchored to a passage or to the course,
//! never part of section generation.
//!
//! A message is stored, then the reply is written from the side-chat context
//! (the passage and the full text of its section's current version when the
//! chat is anchored, the index titles, the profile and the thread) with web
//! search on, streamed as generation events ([`reply`]). A complete reply is
//! stored with its citations and sources and its follow-up prompts are
//! offered; a stopped one is stored as far as it arrived, marked stopped; a
//! failed one stores nothing. One reply streams per chat at a time.
//!
//! A side chat reads the course but writes only its own chat and messages:
//! no section, version, record, convention, map or profile, and nothing in
//! it reaches the course memory sections are written from (ADR-0005,
//! ADR-0013). The prompts offered last are kept for the session only.

mod follow_ups;
mod reply;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::Notify;
use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::harness::chat_context::{ChatContext, ChatPassage, ChatTurn, side_chat_context};
use crate::harness::context::Budget;
use crate::harness::prompts;
use crate::ports::EventSink;
use crate::protocol::{
    Anchor, BlockContent, ChatMessage, ChatRole, CoreError, CoreEvent, GenerationEvent,
    GenerationOutcome, GenerationTarget, Goal, MessageState, SideChat, SideChatSummary,
};
use crate::provider::{ModelProvider, ProviderRequest, Task};
use crate::store::{NewChatMessage, SideChatRecord, Store, StoredSectionVersion};
use reply::{Emitter, Reply};

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
    events: Arc<dyn EventSink>,
    /// The reply streaming in each chat.
    running: Mutex<HashMap<Uuid, Arc<Stop>>>,
    /// The prompts offered after each chat's last reply, this session.
    follow_ups: Mutex<HashMap<Uuid, Vec<String>>>,
    /// The route refused the search tool this session: replies are written
    /// without it.
    search_refused: AtomicBool,
}

/// A request to stop a reply. It holds once made, so a stop that comes
/// before the stream waits for it is not lost.
#[derive(Default)]
struct Stop {
    requested: AtomicBool,
    signal: Notify,
}

impl Stop {
    fn request(&self) {
        self.requested.store(true, Ordering::Release);
        self.signal.notify_one();
    }

    /// Resolves once a stop has been requested.
    async fn requested(&self) {
        while !self.requested.load(Ordering::Acquire) {
            self.signal.notified().await;
        }
    }
}

/// The reply streaming in a chat. Dropped, finished or not, it takes the
/// reply off the running ones.
struct Running<'a> {
    running: &'a Mutex<HashMap<Uuid, Arc<Stop>>>,
    chat: Uuid,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        lock(self.running).remove(&self.chat);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("side chats lock poisoned")
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
            events: deps.events.clone(),
            running: Mutex::default(),
            follow_ups: Mutex::default(),
            search_refused: AtomicBool::new(false),
        })
    }

    /// The course's side chats, newest first.
    pub(crate) async fn list(
        &self,
        _cx: &Features,
        course_id: Uuid,
    ) -> Result<Vec<SideChatSummary>, CoreError> {
        self.require_course(course_id)?;
        self.store
            .side_chats(course_id)?
            .iter()
            .map(|chat| self.summary(chat))
            .collect()
    }

    /// A new chat, tied to the anchored passage or to the course alone.
    pub(crate) async fn start(
        &self,
        _cx: &Features,
        course_id: Uuid,
        anchor: Option<Anchor>,
    ) -> Result<SideChat, CoreError> {
        self.require_course(course_id)?;
        if let Some(anchor) = &anchor {
            self.check_anchor(course_id, anchor)?;
        }
        let chat = self.store.start_side_chat(course_id, anchor.as_ref())?;
        self.announce(&chat)?;
        self.view(&chat)
    }

    /// The chat with its thread.
    pub(crate) async fn open(&self, _cx: &Features, chat_id: Uuid) -> Result<SideChat, CoreError> {
        self.view(&self.chat(chat_id)?)
    }

    /// Stores the candidate's message and streams the reply. A complete reply
    /// is stored and its follow-up prompts offered; a stopped one is stored
    /// as far as it arrived, marked stopped, and answers; a failed one stores
    /// nothing and fails.
    pub(crate) async fn send(
        &self,
        cx: &Features,
        chat_id: Uuid,
        text: String,
    ) -> Result<ChatMessage, CoreError> {
        let chat = self.chat(chat_id)?;
        let text = text.trim();
        if text.is_empty() {
            return Err(CoreError::InvalidRequest {
                message: "the message is empty".to_owned(),
            });
        }
        let (stop, _running) = self.claim(chat_id)?;
        let model = cx.models.current(cx).await?;
        self.store.add_chat_message(
            chat_id,
            &NewChatMessage {
                role: ChatRole::Candidate,
                text: text.to_owned(),
                state: MessageState::Complete,
                citations: Vec::new(),
                sources: Vec::new(),
            },
        )?;
        lock(&self.follow_ups).remove(&chat_id);
        self.announce(&chat)?;

        let request = ProviderRequest {
            model,
            ..ProviderRequest::new(
                Task::SideChatReply,
                prompts::SIDE_CHAT_REPLY.text(),
                self.context(&chat)?.input(),
            )
        };
        let emit = Emitter {
            events: self.events.as_ref(),
            generation_id: Uuid::new_v4(),
            target: GenerationTarget::SideChat {
                course_id: chat.course_id,
                chat_id,
            },
        };
        emit.event(GenerationEvent::Started);
        let mut reply = Reply::default();
        let written = tokio::select! {
            biased;
            () = stop.requested() => Ok(false),
            written = self.ground(&request, &emit, &mut reply) => written.map(|()| true),
        };
        let complete = match written {
            Ok(complete) => complete,
            Err(ended) => {
                let (outcome, error) = ended.into_outcome();
                emit.event(GenerationEvent::Finished { outcome });
                return Err(error);
            }
        };

        let replied = reply.finish(&emit, complete);
        let stored = self.store.add_chat_message(
            chat_id,
            &NewChatMessage {
                role: ChatRole::Agent,
                text: replied.text,
                state: if complete {
                    MessageState::Complete
                } else {
                    MessageState::Stopped
                },
                citations: replied.citations,
                sources: replied.sources,
            },
        );
        let message = match stored {
            Ok(message) => message,
            Err(error) => {
                let error = CoreError::from(error);
                emit.event(GenerationEvent::Finished {
                    outcome: GenerationOutcome::Failed {
                        error: error.clone(),
                    },
                });
                return Err(error);
            }
        };
        emit.event(GenerationEvent::Finished {
            outcome: if complete {
                GenerationOutcome::Completed
            } else {
                GenerationOutcome::Stopped
            },
        });
        if complete {
            lock(&self.follow_ups).insert(chat_id, replied.follow_ups.clone());
            self.events.emit(CoreEvent::FollowUpPrompts {
                chat_id,
                prompts: replied.follow_ups,
            });
        }
        self.announce(&chat)?;
        Ok(message)
    }

    /// Stops the reply streaming in the chat, if one is.
    pub(crate) async fn stop_reply(&self, _cx: &Features, chat_id: Uuid) -> Result<(), CoreError> {
        self.chat(chat_id)?;
        if let Some(stop) = lock(&self.running).get(&chat_id) {
            stop.request();
        }
        Ok(())
    }

    /// Removes a side chat with its messages; not built yet.
    pub(crate) async fn delete(&self, _cx: &Features, _chat_id: Uuid) -> Result<(), CoreError> {
        Err(CoreError::Unavailable {
            feature: "side_chats".to_owned(),
        })
    }

    /// Takes the chat's reply slot, unless a reply is streaming in it.
    fn claim(&self, chat: Uuid) -> Result<(Arc<Stop>, Running<'_>), CoreError> {
        let mut running = lock(&self.running);
        if running.contains_key(&chat) {
            return Err(CoreError::InvalidRequest {
                message: "a reply is still streaming in this side chat".to_owned(),
            });
        }
        let stop = Arc::new(Stop::default());
        running.insert(chat, stop.clone());
        Ok((
            stop,
            Running {
                running: &self.running,
                chat,
            },
        ))
    }

    /// The context the chat's next reply is written from, its thread as
    /// stored.
    fn context(&self, chat: &SideChatRecord) -> Result<ChatContext, CoreError> {
        let course = chat.course_id;
        let record = self
            .store
            .course_record(course)?
            .ok_or_else(|| course_not_found(course))?;
        let ladder = self
            .store
            .ladder(course)?
            .unwrap_or(Ladder { rungs: Vec::new() });
        // A course placed without a profile is answered for a candidate who
        // knows no rung.
        let profile = self
            .store
            .profile(course)?
            .unwrap_or_else(|| CandidateProfile {
                goal: record.goal.unwrap_or(Goal::Overview),
                rungs: Vec::new(),
                misconceptions: Vec::new(),
            });
        let index = self.store.index_tree(course)?;
        let section = match &chat.anchor {
            Some(anchor) => Some(self.section_text(anchor)?),
            None => None,
        };
        let passage = chat
            .anchor
            .as_ref()
            .zip(section.as_deref())
            .map(|(anchor, section_text)| ChatPassage {
                quote: &anchor.quote,
                section_text,
            });
        let history: Vec<ChatTurn> = self
            .store
            .chat_messages(chat.id)?
            .into_iter()
            .map(|message| ChatTurn {
                role: message.role,
                text: message.text,
                stopped: message.state == MessageState::Stopped,
            })
            .collect();
        Ok(side_chat_context(
            passage,
            &index,
            &ladder,
            &profile,
            &history,
            Budget::SIDE_CHAT,
        ))
    }

    /// The full text of the current version of the anchored passage's
    /// section, under its title.
    fn section_text(&self, anchor: &Anchor) -> Result<String, CoreError> {
        let (version, title) = self.anchored(anchor)?;
        let current = self.store.current_version(version.node_id)?;
        Ok(render_version(&title, &current.unwrap_or(version)))
    }

    /// The anchored version and its section's title.
    fn anchored(&self, anchor: &Anchor) -> Result<(StoredSectionVersion, String), CoreError> {
        let not_found = || CoreError::NotFound {
            what: format!("version {}", anchor.version_id),
        };
        let version = self
            .store
            .version(anchor.version_id)?
            .ok_or_else(not_found)?;
        let section = self
            .store
            .index_node(version.node_id)?
            .ok_or_else(not_found)?;
        Ok((version, section.title))
    }

    /// The anchor names a block of a version of a section of the course, and
    /// quotes some text.
    fn check_anchor(&self, course_id: Uuid, anchor: &Anchor) -> Result<(), CoreError> {
        let not_found = || CoreError::NotFound {
            what: format!("block {} of version {}", anchor.block, anchor.version_id),
        };
        let version = self
            .store
            .version(anchor.version_id)?
            .ok_or_else(not_found)?;
        let in_course = self
            .store
            .index_node(version.node_id)?
            .is_some_and(|section| section.course_id == course_id);
        if !in_course
            || !version
                .blocks
                .iter()
                .any(|block| block.index == anchor.block)
        {
            return Err(not_found());
        }
        if anchor.quote.trim().is_empty() {
            return Err(CoreError::InvalidRequest {
                message: "no passage is selected".to_owned(),
            });
        }
        Ok(())
    }

    fn require_course(&self, course_id: Uuid) -> Result<(), CoreError> {
        match self.store.course_record(course_id)? {
            Some(_) => Ok(()),
            None => Err(course_not_found(course_id)),
        }
    }

    fn chat(&self, chat_id: Uuid) -> Result<SideChatRecord, CoreError> {
        self.store
            .side_chat(chat_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("side chat {chat_id}"),
            })
    }

    /// Reports the chat as it stands now.
    fn announce(&self, chat: &SideChatRecord) -> Result<(), CoreError> {
        let chat = self.chat(chat.id)?;
        self.events.emit(CoreEvent::SideChatsChanged {
            course_id: chat.course_id,
            chat: self.summary(&chat)?,
        });
        Ok(())
    }

    /// The chat as listed: its passage, or without one its first question.
    fn summary(&self, chat: &SideChatRecord) -> Result<SideChatSummary, CoreError> {
        let opening = match &chat.anchor {
            Some(anchor) => anchor.quote.clone(),
            None => self
                .store
                .chat_messages(chat.id)?
                .into_iter()
                .find(|message| message.role == ChatRole::Candidate)
                .map(|message| message.text)
                .unwrap_or_default(),
        };
        Ok(SideChatSummary {
            id: chat.id,
            course_id: chat.course_id,
            anchor: chat.anchor.clone(),
            section_title: self.section_title(chat)?,
            opening,
            created_at: chat.created_at,
            updated_at: chat.updated_at,
        })
    }

    fn view(&self, chat: &SideChatRecord) -> Result<SideChat, CoreError> {
        Ok(SideChat {
            id: chat.id,
            course_id: chat.course_id,
            anchor: chat.anchor.clone(),
            section_title: self.section_title(chat)?,
            messages: self.store.chat_messages(chat.id)?,
            follow_ups: lock(&self.follow_ups)
                .get(&chat.id)
                .cloned()
                .unwrap_or_default(),
            created_at: chat.created_at,
            updated_at: chat.updated_at,
        })
    }

    /// The title of the anchored passage's section, if any.
    fn section_title(&self, chat: &SideChatRecord) -> Result<Option<String>, CoreError> {
        chat.anchor
            .as_ref()
            .map(|anchor| self.anchored(anchor).map(|(_, title)| title))
            .transpose()
    }
}

fn course_not_found(course_id: Uuid) -> CoreError {
    CoreError::NotFound {
        what: format!("course {course_id}"),
    }
}

/// A version as text: its title, then each block, prose as written and
/// every other block by what it shows.
fn render_version(title: &str, version: &StoredSectionVersion) -> String {
    let mut parts = vec![format!("# {title}")];
    for block in &version.blocks {
        let text = match &block.content {
            BlockContent::Prose { text } => text.trim().to_owned(),
            BlockContent::Visual { spec, description } => match description {
                Some(description) => format!("[Visual: {description}]"),
                None => format!("[Visual]\n{spec}"),
            },
            BlockContent::Figure { description, .. } => {
                format!(
                    "[Figure: {}]",
                    description.as_deref().unwrap_or("no description")
                )
            }
            BlockContent::Mermaid { source, .. } => format!("```mermaid\n{}\n```", source.trim()),
            BlockContent::Check { questions } => {
                let mut text = "[Check questions]".to_owned();
                for question in questions {
                    text.push_str(&format!("\n{}", question.stem));
                    for option in &question.options {
                        text.push_str(&format!("\n- {option}"));
                    }
                }
                text
            }
            BlockContent::Fallback { description } => format!("[Visual: {description}]"),
        };
        parts.push(text);
    }
    parts.join("\n\n")
}
