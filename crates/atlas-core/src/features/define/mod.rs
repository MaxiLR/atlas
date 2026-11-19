//! Define: a short explanation of selected text, kept in the course glossary.
//!
//! One plain request per anchor: the selected text, the sentence around it
//! and the candidate profile, with no history, no conversation and no web
//! search. The text streams as generation events and is stored only once the
//! provider reports completion; an anchor already defined answers from the
//! store.

use std::sync::Arc;

use futures::StreamExt;
use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::harness::context::render_profile;
use crate::harness::prompts;
use crate::ports::EventSink;
use crate::protocol::{
    Activity, Anchor, Block, BlockContent, BlockKind, CoreError, CoreEvent, GenerationEvent,
    GenerationOutcome, GenerationTarget, GlossaryEntry,
};
use crate::provider::{ModelProvider, ProviderError, ProviderEvent, ProviderRequest, Task};
use crate::store::{NewGlossaryEntry, Store};

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
    events: Arc<dyn EventSink>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
            events: deps.events.clone(),
        })
    }

    /// The stored entry for exactly this anchor, or a definition written
    /// for it, streamed, stored and announced once complete.
    pub(crate) async fn define(
        &self,
        cx: &Features,
        anchor: Anchor,
        sentence: String,
    ) -> Result<GlossaryEntry, CoreError> {
        if let Some(entry) = self.store.glossary_entry_for_anchor(&anchor)? {
            return Ok(entry);
        }
        let term = anchor.quote.trim().to_owned();
        if term.is_empty() {
            return Err(CoreError::InvalidRequest {
                message: "nothing is selected to define".to_owned(),
            });
        }
        let course_id = self.course_of(&anchor)?;
        let request = ProviderRequest {
            model: cx.models.current(cx).await?,
            ..ProviderRequest::new(
                Task::Define,
                prompts::DEFINE.text(),
                self.input(course_id, &term, &sentence)?,
            )
        };

        let target = GenerationTarget::Define {
            course_id,
            anchor: anchor.clone(),
        };
        let definition = self.generate(target, request).await?;
        let entry = self.store.add_glossary_entry(&NewGlossaryEntry {
            course: course_id,
            term,
            definition,
            anchor,
        })?;
        self.events.emit(CoreEvent::GlossaryChanged {
            course_id,
            entry: entry.clone(),
        });
        Ok(entry)
    }

    /// Every entry of the course, in the order defined, with the title of
    /// the section it came from.
    pub(crate) async fn list_glossary(
        &self,
        _cx: &Features,
        course_id: Uuid,
    ) -> Result<Vec<GlossaryEntry>, CoreError> {
        if self.store.course_record(course_id)?.is_none() {
            return Err(CoreError::NotFound {
                what: format!("course {course_id}"),
            });
        }
        Ok(self.store.glossary(course_id)?)
    }

    /// The course the anchored block belongs to.
    fn course_of(&self, anchor: &Anchor) -> Result<Uuid, CoreError> {
        let not_found = || CoreError::NotFound {
            what: format!("block {} of version {}", anchor.block, anchor.version_id),
        };
        let version = self
            .store
            .version(anchor.version_id)?
            .ok_or_else(not_found)?;
        if !version
            .blocks
            .iter()
            .any(|block| block.index == anchor.block)
        {
            return Err(not_found());
        }
        let section = self
            .store
            .index_node(version.node_id)?
            .ok_or_else(not_found)?;
        Ok(section.course_id)
    }

    /// The topic, the selection, its sentence and the profile; nothing from
    /// any section's text, record or side chat.
    fn input(&self, course_id: Uuid, term: &str, sentence: &str) -> Result<String, CoreError> {
        let course = self
            .store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })?;
        let mut input = format!(
            "Topic: {}\n\nSelected text: {term}\n\nSentence: {}",
            course.topic,
            sentence.trim()
        );
        if let (Some(ladder), Some(profile)) = (
            self.store.ladder(course_id)?,
            self.store.profile(course_id)?,
        ) {
            input.push_str("\n\nCandidate profile:\n");
            input.push_str(&render_profile(&ladder, &profile));
        }
        Ok(input)
    }

    /// Streams the definition as one prose block and returns its text once
    /// the provider completes; the events end with the outcome either way.
    async fn generate(
        &self,
        target: GenerationTarget,
        request: ProviderRequest,
    ) -> Result<String, CoreError> {
        let generation_id = Uuid::new_v4();
        let emit = |event| {
            self.events.emit(CoreEvent::Generation {
                generation_id,
                target: target.clone(),
                event,
            });
        };
        emit(GenerationEvent::Started);
        emit(GenerationEvent::Activity {
            activity: Activity::Writing,
        });
        emit(GenerationEvent::BlockOpened {
            block: 0,
            kind: BlockKind::Prose,
        });
        let (outcome, written) = match self.stream(request, &emit).await {
            Ok(Written::Completed(text)) => {
                let text = text.trim().to_owned();
                emit(GenerationEvent::BlockCompleted {
                    block: Block {
                        index: 0,
                        content: BlockContent::Prose { text: text.clone() },
                    },
                });
                (GenerationOutcome::Completed, Ok(text))
            }
            Ok(Written::Incomplete(reason)) => {
                let error = ProviderError::failed(format!(
                    "the response is incomplete: {}",
                    reason.as_deref().unwrap_or("no reason given")
                ));
                (GenerationOutcome::Incomplete { reason }, Err(error.into()))
            }
            Err(error) => {
                let error = CoreError::from(error);
                (
                    GenerationOutcome::Failed {
                        error: error.clone(),
                    },
                    Err(error),
                )
            }
        };
        emit(GenerationEvent::Finished { outcome });
        written
    }

    /// Runs the request, passing each step on as a generation event.
    async fn stream(
        &self,
        request: ProviderRequest,
        emit: &(impl Fn(GenerationEvent) + Sync),
    ) -> Result<Written, ProviderError> {
        let mut stream = self.provider.stream(request).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event? {
                ProviderEvent::TextDelta(delta) => {
                    text.push_str(&delta);
                    emit(GenerationEvent::TextDelta {
                        block: 0,
                        text: delta,
                    });
                }
                ProviderEvent::Retrying { attempt } => {
                    text.clear();
                    emit(GenerationEvent::Activity {
                        activity: Activity::Retrying { attempt },
                    });
                }
                ProviderEvent::Completed => return Ok(Written::Completed(text)),
                ProviderEvent::Incomplete { reason } => return Ok(Written::Incomplete(reason)),
                // No search is asked for, so none is reported.
                ProviderEvent::Searching | ProviderEvent::Citation(_) => {}
            }
        }
        Err(ProviderError::Incomplete { request_id: None })
    }
}

/// How a stream that did not fail ended.
enum Written {
    Completed(String),
    Incomplete(Option<String>),
}
