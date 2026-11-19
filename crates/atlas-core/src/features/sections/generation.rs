//! One generation of a section version.
//!
//! The request is the write-section prompt with the catalog's instructions,
//! and the context the core selects from the course memory loaded from the
//! store. The stream goes through the splitter: prose is reported as it
//! arrives, each shown block is opened as a placeholder and completed once
//! validated, repaired or replaced by its fallback, and the record block
//! never leaves the core. The generation is complete once the provider
//! reported completion, every shown block is resolved and the record is
//! resolved; only then is anything stored, in one transaction. A stop drops
//! the stream and whatever repair is under way at once.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use futures::StreamExt;
use uuid::Uuid;

use super::{Feature, Features, Outcome, Stop, stopped};
use crate::course_map;
use crate::domain::conventions::ConventionOrigin;
use crate::domain::course_map::MapEntry;
use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::domain::record::StoredRecord;
use crate::harness::context::{Budget, CourseMemory, IndexKeys, section_context};
use crate::harness::record::{apply_conventions, resolve as resolve_record};
use crate::harness::splitter::{ReservedKind, SplitEvent, Splitter};
use crate::harness::validate::{RawBlock, Resolved, ValidBlock, resolve as resolve_block};
use crate::harness::{catalog, prompts};
use crate::index_tree::in_index_order;
use crate::ports::EventSink;
use crate::protocol::{
    Activity, Block, BlockContent, BlockKind, CoreError, CoreEvent, GenerationEvent,
    GenerationOutcome, GenerationTarget, Goal, VersionStyle,
};
use crate::provider::{ProviderError, ProviderEvent, ProviderRequest, Task};
use crate::store::{CheckBlock, NewSectionVersion, NodeRecord, StoreError, StoredSectionVersion};

/// Everything a generation needs, gathered before it starts.
struct Plan {
    node: Uuid,
    course: Uuid,
    style: VersionStyle,
    request: ProviderRequest,
    ladder: Ladder,
    /// The section's map entry; empty when the map has none for it.
    entry: MapEntry,
    /// The index keys of the sections after it, which a promise may name.
    later: Vec<String>,
}

/// How a generation ended without a version to store.
enum Ended {
    Stopped,
    Incomplete(Option<String>),
    Failed(CoreError),
}

impl From<CoreError> for Ended {
    fn from(error: CoreError) -> Self {
        Self::Failed(error)
    }
}

impl From<ProviderError> for Ended {
    fn from(error: ProviderError) -> Self {
        Self::Failed(error.into())
    }
}

impl From<StoreError> for Ended {
    fn from(error: StoreError) -> Self {
        Self::Failed(error.into())
    }
}

impl Ended {
    /// The outcome the events end with, and the error the request fails with.
    fn into_outcome(self) -> (GenerationOutcome, CoreError) {
        match self {
            Self::Stopped => (GenerationOutcome::Stopped, stopped()),
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
        }
    }
}

/// A complete section, ready to store.
struct Written {
    blocks: Vec<Block>,
    checks: Vec<CheckBlock>,
    record: StoredRecord,
}

/// The section as it arrives: the blocks resolved so far, the prose block
/// still open and the record's payload.
#[derive(Default)]
struct Section {
    blocks: Vec<Block>,
    checks: Vec<CheckBlock>,
    /// The open prose block and its text so far.
    prose: Option<(u32, String)>,
    record: Option<String>,
}

impl Section {
    fn close_prose(&mut self, emit: &Emitter<'_>) {
        if let Some((index, text)) = self.prose.take() {
            self.complete(
                emit,
                Block {
                    index,
                    content: BlockContent::Prose { text },
                },
            );
        }
    }

    fn complete(&mut self, emit: &Emitter<'_>, block: Block) {
        emit.event(GenerationEvent::BlockCompleted {
            block: block.clone(),
        });
        self.blocks.push(block);
    }

    /// The section's own prose, for the record's repair.
    fn text(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|block| match &block.content {
                BlockContent::Prose { text } => Some(text.trim()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Emits the events of one generation.
struct Emitter<'a> {
    events: &'a dyn EventSink,
    generation_id: Uuid,
    target: GenerationTarget,
}

impl Emitter<'_> {
    fn event(&self, event: GenerationEvent) {
        self.events.emit(CoreEvent::Generation {
            generation_id: self.generation_id,
            target: self.target.clone(),
            event,
        });
    }
}

/// The kind a shown block is opened as; none for the record.
fn shown_kind(kind: ReservedKind) -> Option<BlockKind> {
    match kind {
        ReservedKind::Visual => Some(BlockKind::Visual),
        ReservedKind::Figure => Some(BlockKind::Figure),
        ReservedKind::Mermaid => Some(BlockKind::Mermaid),
        ReservedKind::Check => Some(BlockKind::Check),
        ReservedKind::Record => None,
    }
}

impl Feature {
    /// Generates a version of `node` and stores it once complete; then the
    /// section is read. Nothing is stored otherwise, and the events end with
    /// how the generation ended either way.
    pub(super) async fn write(
        &self,
        cx: &Features,
        node: &NodeRecord,
        style: VersionStyle,
        parent: Option<Uuid>,
        stop: &Stop,
    ) -> Outcome {
        let plan = self.plan(cx, node, style, parent).await?;
        let emit = Emitter {
            events: self.events.as_ref(),
            generation_id: Uuid::new_v4(),
            target: GenerationTarget::Section {
                course_id: plan.course,
                section_id: plan.node,
                style,
            },
        };
        emit.event(GenerationEvent::Started);
        let written = tokio::select! {
            biased;
            () = stop.requested() => Err(Ended::Stopped),
            written = self.stream(&plan, &emit, &stop.requested) => written,
        };
        match written.and_then(|written| Ok(self.store_version(&plan, written)?)) {
            Ok(version) => {
                emit.event(GenerationEvent::Finished {
                    outcome: GenerationOutcome::Completed,
                });
                self.mark_read(cx, node)?;
                self.view(node, version)
            }
            Err(ended) => {
                let (outcome, error) = ended.into_outcome();
                emit.event(GenerationEvent::Finished { outcome });
                Err(error)
            }
        }
    }

    /// The request and what the stream is checked against, from the course
    /// memory in the store.
    async fn plan(
        &self,
        cx: &Features,
        node: &NodeRecord,
        style: VersionStyle,
        parent: Option<Uuid>,
    ) -> Result<Plan, CoreError> {
        let course = node.course_id;
        let record = self
            .store
            .course_record(course)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course}"),
            })?;
        let ladder = self
            .store
            .ladder(course)?
            .unwrap_or(Ladder { rungs: Vec::new() });
        // A course placed without a profile is written for a candidate who
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
        let map = self.store.course_map(course)?;
        let conventions = self.store.conventions(course)?;

        let order = in_index_order(&index);
        let mut records = HashMap::new();
        for entry in &order {
            if let Some(stored) = self.store.current_record(entry.section.id)? {
                records.insert(entry.section.id, stored);
            }
        }
        let keys = IndexKeys::new(&index);
        let later = order
            .iter()
            .skip_while(|entry| entry.section.id != node.id)
            .skip(1)
            .filter_map(|entry| keys.key(entry.section.id).map(str::to_owned))
            .collect();
        let entry = course_map::entry(&map, node.id)
            .cloned()
            .unwrap_or_else(|| MapEntry {
                section: node.id,
                introduces: Vec::new(),
                relies_on: Vec::new(),
            });

        let memory = CourseMemory {
            ladder,
            profile,
            index,
            map,
            conventions,
            records,
        };
        let context = section_context(&memory, node.id, Budget::SECTION).ok_or_else(|| {
            CoreError::NotFound {
                what: format!("section {} in its course's index", node.id),
            }
        })?;
        let mut input = context.input();
        if let Some(parent) = parent {
            input.push_str("\n\n");
            input.push_str(&self.rewritten(node, parent)?);
        }
        let instructions = format!(
            "{}\n\n## Catalog visuals\n\n{}",
            prompts::WRITE_SECTION.text(),
            catalog::prompt()
        );
        Ok(Plan {
            node: node.id,
            course,
            style,
            request: ProviderRequest {
                model: cx.models.current(cx).await?,
                ..ProviderRequest::new(Task::WriteSection, instructions, input)
            },
            ladder: memory.ladder,
            entry,
            later,
        })
    }

    /// The record of the version `parent` of `node`, which the new version
    /// rewrites, as the request carries it.
    fn rewritten(&self, node: &NodeRecord, parent: Uuid) -> Result<String, CoreError> {
        let version = self
            .store
            .version(parent)?
            .filter(|version| version.node_id == node.id)
            .ok_or_else(|| CoreError::NotFound {
                what: format!("version {parent} of section {}", node.id),
            })?;
        let record = match version.record {
            Some(stored) => {
                serde_json::to_string(&stored.record).map_err(|error| CoreError::Store {
                    message: error.to_string(),
                })?
            }
            None => "none".to_owned(),
        };
        Ok(format!(
            "# The version this one rewrites\nIts record: {record}"
        ))
    }

    /// Streams the section through the splitter, resolving each shown block
    /// as it closes, then resolves the record. What the agent is doing is
    /// reported as it changes: writing from the start and whenever text
    /// arrives again after a search or a retry.
    async fn stream(
        &self,
        plan: &Plan,
        emit: &Emitter<'_>,
        stopped: &AtomicBool,
    ) -> Result<Written, Ended> {
        let mut stream = self.provider.stream(plan.request.clone()).await?;
        let mut splitter = Splitter::new();
        let mut section = Section::default();
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
                .unwrap_or(Err(ProviderError::Incomplete { request_id: None }))?;
            match event {
                ProviderEvent::TextDelta(text) => {
                    report(Activity::Writing);
                    for split in splitter.push(text.as_bytes()) {
                        self.take(plan, emit, &mut section, split, stopped).await;
                    }
                }
                ProviderEvent::Searching => report(Activity::Searching),
                ProviderEvent::Retrying { attempt } => {
                    splitter = Splitter::new();
                    section = Section::default();
                    report(Activity::Retrying { attempt });
                }
                // Citations are not kept yet.
                ProviderEvent::Citation(_) => {}
                ProviderEvent::Completed => break,
                ProviderEvent::Incomplete { reason } => return Err(Ended::Incomplete(reason)),
            }
        }
        drop(stream);
        for split in splitter.finish() {
            self.take(plan, emit, &mut section, split, stopped).await;
        }

        // A section deleted while it was written is not stored, so no repair
        // is spent on its record.
        if self.store.index_node(plan.node)?.is_none() {
            return Err(Ended::Failed(super::not_found(plan.node)));
        }
        let record = resolve_record(
            section.record.as_deref(),
            &section.text(),
            &plan.entry,
            &plan.later,
            self.provider.as_ref(),
            plan.request.model.as_deref(),
        )
        .await;
        Ok(Written {
            blocks: section.blocks,
            checks: section.checks,
            record,
        })
    }

    /// Reports one step of the splitter and keeps what it completes.
    async fn take(
        &self,
        plan: &Plan,
        emit: &Emitter<'_>,
        section: &mut Section,
        split: SplitEvent,
        stopped: &AtomicBool,
    ) {
        match split {
            SplitEvent::Prose { block, text } => {
                if section.prose.as_ref().map(|(index, _)| *index) != Some(block) {
                    section.close_prose(emit);
                    section.prose = Some((block, String::new()));
                    emit.event(GenerationEvent::BlockOpened {
                        block,
                        kind: BlockKind::Prose,
                    });
                }
                if let Some((_, arrived)) = &mut section.prose {
                    arrived.push_str(&text);
                }
                emit.event(GenerationEvent::TextDelta { block, text });
            }
            SplitEvent::Opened { block, kind } => {
                if let (Some(block), Some(kind)) = (block, shown_kind(kind)) {
                    section.close_prose(emit);
                    emit.event(GenerationEvent::BlockOpened { block, kind });
                }
            }
            SplitEvent::Closed {
                block,
                kind,
                payload,
                description,
            } => match (block, RawBlock::closed(kind, payload.as_str(), description)) {
                (Some(block), Ok(raw)) => {
                    self.resolve(plan, emit, section, block, raw, stopped).await
                }
                _ => section.record = Some(payload),
            },
            SplitEvent::Unclosed {
                block,
                kind,
                payload,
                description,
            } => match (
                block,
                RawBlock::unclosed(kind, payload.as_str(), description),
            ) {
                (Some(block), Ok(raw)) => {
                    self.resolve(plan, emit, section, block, raw, stopped).await
                }
                _ => section.record = Some(payload),
            },
            SplitEvent::Finished { .. } => section.close_prose(emit),
        }
    }

    /// Completes a shown block: valid, repaired once, or its text fallback.
    async fn resolve(
        &self,
        plan: &Plan,
        emit: &Emitter<'_>,
        section: &mut Section,
        index: u32,
        raw: RawBlock,
        stopped: &AtomicBool,
    ) {
        let resolved = resolve_block(
            &raw,
            &plan.ladder,
            self.provider.as_ref(),
            plan.request.model.as_deref(),
            stopped,
        )
        .await;
        if let Resolved::Block {
            block: ValidBlock::Check { questions },
            ..
        } = &resolved
        {
            section.checks.push(CheckBlock {
                block: index,
                questions: questions.clone(),
            });
        }
        section.complete(
            emit,
            Block {
                index,
                content: resolved.content(),
            },
        );
    }

    /// Stores the version with its record and the conventions the record
    /// newly fixes, in one transaction that also reads the conventions
    /// already fixed; the record's entries that conflict with one are left
    /// out of both.
    fn store_version(
        &self,
        plan: &Plan,
        written: Written,
    ) -> Result<StoredSectionVersion, CoreError> {
        let id = Uuid::new_v4();
        let Written {
            blocks,
            checks,
            record,
        } = written;
        self.store.insert_version_with(plan.node, |existing| {
            let applied = apply_conventions(
                record.record,
                existing,
                ConventionOrigin::SectionVersion { version: id },
            );
            let version = NewSectionVersion {
                id,
                node: plan.node,
                style: plan.style,
                blocks,
                checks,
                citations: Vec::new(),
                sources: Vec::new(),
                written_without_search: false,
                record: StoredRecord {
                    record: applied.record,
                    status: record.status,
                },
                model: plan.request.model.clone(),
                prompt: prompts::WRITE_SECTION.id(),
            };
            (version, applied.fixed)
        })?;
        self.store
            .version(id)?
            .ok_or_else(|| super::not_found(plan.node))
    }
}
