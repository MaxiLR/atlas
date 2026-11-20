//! One reply streamed: the text shown as one prose block as it arrives, its
//! follow-up block held back, its citations placed on the text as it is
//! reached, with web search unless the route refused it.

use std::sync::atomic::Ordering;

use futures::StreamExt;
use uuid::Uuid;

use super::Feature;
use super::follow_ups::FollowUps;
use crate::features::sections::grounding::refuses_search;
use crate::harness::citations::Citations;
use crate::ports::EventSink;
use crate::protocol::{
    Activity, Block, BlockContent, BlockKind, Citation, CoreError, CoreEvent, GenerationEvent,
    GenerationOutcome, GenerationTarget, Source,
};
use crate::provider::{ProviderCitation, ProviderError, ProviderEvent, ProviderRequest};

/// Emits the events of one reply.
pub(super) struct Emitter<'a> {
    pub(super) events: &'a dyn EventSink,
    pub(super) generation_id: Uuid,
    pub(super) target: GenerationTarget,
}

impl Emitter<'_> {
    pub(super) fn event(&self, event: GenerationEvent) {
        self.events.emit(CoreEvent::Generation {
            generation_id: self.generation_id,
            target: self.target.clone(),
            event,
        });
    }

    fn citations(&self, placed: Vec<(Citation, Source)>) {
        for (citation, source) in placed {
            self.event(GenerationEvent::CitationAdded { citation, source });
        }
    }
}

/// How a reply ended without being complete or stopped.
pub(super) enum Ended {
    Incomplete(Option<String>),
    Failed(CoreError),
    /// The route refused the search tool; `streamed` tells whether the
    /// stream had begun.
    SearchRefused {
        error: ProviderError,
        streamed: bool,
    },
}

impl Ended {
    /// The outcome the events end with, and the error the request fails with.
    pub(super) fn into_outcome(self) -> (GenerationOutcome, CoreError) {
        match self {
            Self::Incomplete(reason) => {
                let message = match &reason {
                    Some(reason) => format!("the response is incomplete: {reason}"),
                    None => "the response is incomplete".to_owned(),
                };
                (
                    GenerationOutcome::Incomplete { reason },
                    CoreError::Provider { message },
                )
            }
            Self::Failed(error) => (
                GenerationOutcome::Failed {
                    error: error.clone(),
                },
                error,
            ),
            Self::SearchRefused { error, .. } => Self::Failed(error.into()).into_outcome(),
        }
    }
}

/// A reply as it ended, complete or stopped.
pub(super) struct Replied {
    /// The text shown, without the follow-up block and trailing whitespace.
    pub(super) text: String,
    /// Ranges of `text`, on block 0.
    pub(super) citations: Vec<Citation>,
    pub(super) sources: Vec<Source>,
    /// The prompts of a complete reply's follow-up block, at most three.
    pub(super) follow_ups: Vec<String>,
}

/// The reply as it arrives.
#[derive(Default)]
pub(super) struct Reply {
    follow_ups: FollowUps,
    citations: Citations,
    shown: String,
    /// Characters of `shown`.
    length: u32,
}

impl Reply {
    fn push(&mut self, emit: &Emitter<'_>, text: &str) {
        let shown = self.follow_ups.push(text);
        self.show(emit, shown);
    }

    /// Shows `text` as the next text of the reply's block.
    fn show(&mut self, emit: &Emitter<'_>, text: String) {
        if text.is_empty() {
            return;
        }
        let chars = text.chars().count() as u32;
        self.citations.prose(self.length, 0, chars);
        self.length += chars;
        self.shown.push_str(&text);
        emit.event(GenerationEvent::TextDelta { block: 0, text });
        emit.citations(self.citations.ready());
    }

    fn cite(&mut self, emit: &Emitter<'_>, citation: ProviderCitation) {
        self.citations.cite(citation);
        emit.citations(self.citations.ready());
    }

    /// Ends the reply: complete, with the text still held shown and its
    /// follow-up prompts read, or stopped, with what was shown. Every
    /// citation left is placed and the block completed.
    pub(super) fn finish(mut self, emit: &Emitter<'_>, complete: bool) -> Replied {
        let follow_ups = if complete {
            let held = self.follow_ups.finish();
            self.show(emit, held);
            self.follow_ups.prompts()
        } else {
            Vec::new()
        };
        emit.citations(self.citations.finish());
        let text = self.shown.trim_end().to_owned();
        let length = text.chars().count() as u32;
        let placed = self.citations.into_placed();
        let citations = placed
            .citations
            .into_iter()
            .map(|citation| Citation {
                start: citation.start.min(length),
                end: citation.end.min(length),
                ..citation
            })
            .collect();
        emit.event(GenerationEvent::BlockCompleted {
            block: Block {
                index: 0,
                content: BlockContent::Prose { text: text.clone() },
            },
        });
        Replied {
            text,
            citations,
            sources: placed.sources,
            follow_ups,
        }
    }
}

impl Feature {
    /// Writes the reply into `reply` with web search, unless the route
    /// refused it earlier in the session; on a refusal, writes it once more
    /// without search and remembers the refusal. Whatever streamed before
    /// the refusal is void, as after a retry.
    pub(super) async fn ground(
        &self,
        request: &ProviderRequest,
        emit: &Emitter<'_>,
        reply: &mut Reply,
    ) -> Result<(), Ended> {
        emit.event(GenerationEvent::BlockOpened {
            block: 0,
            kind: BlockKind::Prose,
        });
        if !self.search_refused.load(Ordering::Acquire) {
            match self.stream(request, emit, reply, true).await {
                Err(Ended::SearchRefused { streamed, .. }) => {
                    self.search_refused.store(true, Ordering::Release);
                    *reply = Reply::default();
                    if streamed {
                        emit.event(GenerationEvent::Activity {
                            activity: Activity::Retrying { attempt: 2 },
                        });
                    }
                }
                written => return written,
            }
        }
        self.stream(request, emit, reply, false).await
    }

    /// Streams the reply into `reply`, with web search when `search` holds.
    /// What the agent is doing is reported as it changes.
    async fn stream(
        &self,
        request: &ProviderRequest,
        emit: &Emitter<'_>,
        reply: &mut Reply,
        search: bool,
    ) -> Result<(), Ended> {
        let ended = |error: ProviderError, streamed: bool| {
            if search && refuses_search(&error) {
                Ended::SearchRefused { error, streamed }
            } else {
                Ended::Failed(error.into())
            }
        };
        let request = ProviderRequest {
            web_search: search,
            ..request.clone()
        };
        let mut stream = self
            .provider
            .stream(request)
            .await
            .map_err(|error| ended(error, false))?;
        let mut activity = None;
        let mut report = |now: Activity| {
            if activity.as_ref() != Some(&now) {
                emit.event(GenerationEvent::Activity {
                    activity: now.clone(),
                });
                activity = Some(now);
            }
        };
        report(Activity::Writing);
        loop {
            let event = stream
                .next()
                .await
                .unwrap_or(Err(ProviderError::Incomplete { request_id: None }))
                .map_err(|error| ended(error, true))?;
            match event {
                ProviderEvent::TextDelta(text) => {
                    report(Activity::Writing);
                    reply.push(emit, &text);
                }
                ProviderEvent::Searching => report(Activity::Searching),
                ProviderEvent::Retrying { attempt } => {
                    *reply = Reply::default();
                    report(Activity::Retrying { attempt });
                }
                ProviderEvent::Citation(citation) => reply.cite(emit, citation),
                ProviderEvent::Completed => return Ok(()),
                ProviderEvent::Incomplete { reason } => return Err(Ended::Incomplete(reason)),
            }
        }
    }
}
