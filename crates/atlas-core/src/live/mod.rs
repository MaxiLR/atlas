//! The live command: each harness task run once against the real plan, on a
//! throwaway course, with a report for the engineer.
//!
//! The course is started in the core's store and every task goes through
//! [`Core::handle`], as in the app. A task that is not selected but that a
//! selected one needs (a ladder, the index, a written section) is seeded in
//! the store instead, with no request ([`seed`]). The core's provider is
//! wrapped ([`observer`]) so every call is recorded for the report and no
//! call is made beyond one per selected task (two for write section) and the
//! repairs that follow. A task whose selected prerequisite failed is skipped.

mod args;
mod capture;
mod observer;
mod report;
mod seed;

pub use args::{Args, ArgsError, USAGE, parse_args};
pub use capture::Capture;

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::domain::record::{RecordStatus, SectionRecord};
use crate::harness::record;
use crate::harness::splitter::{ReservedKind, SplitEvent, Splitter};
use crate::index_tree::in_index_order;
use crate::protocol::{
    Anchor, BlockContent, CoreError, CoreRequest, CoreResponse, ExplainStyle, ModelList,
    PlacementStep, PlanUsage, Session, UsageState,
};
use crate::provider::Task;
use crate::store::Store;
use crate::{Core, CoreDeps};
use observer::{Call, Observer, Outcome};

/// The tasks the live command can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiveTask {
    DraftLadder,
    PlacementQuestion,
    BuildIndex,
    WriteSection,
    ProposeDeepDives,
    ExplainDifferently,
    Define,
    SideChatReply,
}

impl LiveTask {
    /// Every task, in the order a run takes them.
    pub const ALL: [LiveTask; 8] = [
        LiveTask::DraftLadder,
        LiveTask::PlacementQuestion,
        LiveTask::BuildIndex,
        LiveTask::WriteSection,
        LiveTask::ProposeDeepDives,
        LiveTask::ExplainDifferently,
        LiveTask::Define,
        LiveTask::SideChatReply,
    ];

    /// The name `--task` takes.
    pub fn name(self) -> &'static str {
        match self {
            Self::DraftLadder => "draft-ladder",
            Self::PlacementQuestion => "placement-question",
            Self::BuildIndex => "build-index",
            Self::WriteSection => "write-section",
            Self::ProposeDeepDives => "propose-deep-dives",
            Self::ExplainDifferently => "explain-differently",
            Self::Define => "define",
            Self::SideChatReply => "side-chat-reply",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|task| task.name() == name)
    }

    /// Whether its answer is data, asked for as structured output.
    fn structured(self) -> bool {
        matches!(
            self,
            Self::DraftLadder | Self::PlacementQuestion | Self::BuildIndex | Self::ProposeDeepDives
        )
    }

    /// Whether it is written with web search.
    fn searches(self) -> bool {
        matches!(
            self,
            Self::WriteSection | Self::ExplainDifferently | Self::SideChatReply
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveResult {
    Passed,
    Failed(CoreError),
    /// A task it depends on failed.
    Skipped,
}

/// Whether the route took the structured output a data task asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Structured {
    Accepted,
    /// The route refused it: the schema went in the prompt instead.
    Fallback,
}

/// Whether the route allowed the hosted web search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Search {
    Allowed,
    /// Refused: written without it.
    Refused,
}

/// Whether the course map, or its extension, held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    AtOnce,
    AfterRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordState {
    Confirmed,
    /// The model's record failed twice; the predicted one was stored.
    Unconfirmed,
}

/// What a task left in the course memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Memory {
    /// Build index: the course map.
    Map {
        valid: Validity,
        concepts: usize,
        conventions: usize,
    },
    /// Write section and explain differently: one per version written.
    Record {
        /// The section's title.
        section: String,
        record: RecordState,
        /// The record's notation and term entries left out as conflicting.
        rejected: usize,
        /// The size of the request's input.
        input_chars: usize,
    },
    /// Propose deep dives: the map's extension.
    Extension { valid: Validity },
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReport {
    pub task: LiveTask,
    pub result: LiveResult,
    /// For a data task that made a request.
    pub structured: Option<Structured>,
    /// For a task written with search that made a request.
    pub search: Option<Search>,
    /// The model its requests named.
    pub model: Option<String>,
    pub repairs: u32,
    /// The time its requests took, repairs included.
    pub took: Duration,
    pub memory: Vec<Memory>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The models listed, in the order the plan reports them.
    pub models: Result<ModelList, CoreError>,
    pub tasks: Vec<TaskReport>,
    /// The usage meter after the run, read with the plan login.
    pub usage: Result<UsageState, CoreError>,
    /// The streams written, when the run captured them.
    pub captured: Option<Result<Vec<PathBuf>, String>>,
}

/// Why a run did not start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LiveError {
    #[error("sign in with ChatGPT in the Atlas app first, then run the live command again")]
    SignInFirst,
    #[error("plan usage is not allowed for this account")]
    PlanNotAllowed,
    #[error(transparent)]
    Core(#[from] CoreError),
}

/// The core a run goes through: built from the deps with its provider
/// wrapped, and a second handle on its store for seeding and reading back.
pub struct LiveCore {
    core: Core,
    store: Store,
    observer: Arc<Observer>,
    capture: Option<Arc<Capture>>,
    model: Option<String>,
}

impl LiveCore {
    /// The core on `deps`, whose database should be a throwaway one.
    pub fn new(deps: CoreDeps) -> Result<Self, CoreError> {
        let observer = Arc::new(Observer::new(deps.provider.clone()));
        let path = deps.database_path.clone();
        let clock = deps.clock.clone();
        let core = Core::new(CoreDeps {
            provider: observer.clone(),
            ..deps
        })?;
        let store = Store::open(&path, clock)?;
        Ok(Self {
            core,
            store,
            observer,
            capture: None,
            model: None,
        })
    }

    /// Writes the streams `capture` receives (the same capture the provider
    /// was given) once the run ends.
    pub fn with_capture(self, capture: Arc<Capture>) -> Self {
        Self {
            capture: Some(capture),
            ..self
        }
    }

    /// Uses `slug` instead of the first model listed.
    pub fn with_model(self, slug: Option<String>) -> Self {
        Self {
            model: slug,
            ..self
        }
    }

    /// Answers once a Codex login is saved and its plan usage is not
    /// refused; reads only the secret store.
    pub async fn require_login(&self) -> Result<(), LiveError> {
        match self.core.handle(CoreRequest::GetSession).await? {
            CoreResponse::Session {
                session:
                    Session::SignedIn {
                        plan_usage: PlanUsage::On,
                        ..
                    },
            } => Ok(()),
            CoreResponse::Session {
                session:
                    Session::SignedIn {
                        plan_usage: PlanUsage::NotAllowed,
                        ..
                    },
            } => Err(LiveError::PlanNotAllowed),
            _ => Err(LiveError::SignInFirst),
        }
    }

    async fn handle(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        self.core.handle(request).await
    }
}

/// Runs each of `tasks` once, in the order of [`LiveTask::ALL`], reporting
/// progress to `out`. Without a plan login it stops before any request.
pub async fn run(
    tasks: &[LiveTask],
    core: &LiveCore,
    out: &mut dyn Write,
) -> Result<Report, LiveError> {
    core.require_login().await?;
    let selected: Vec<LiveTask> = LiveTask::ALL
        .into_iter()
        .filter(|task| tasks.contains(task))
        .collect();
    core.observer.allow(&selected);
    if let Some(slug) = &core.model {
        core.handle(CoreRequest::SelectModel { slug: slug.clone() })
            .await?;
    }
    let models = core
        .handle(CoreRequest::ListModels)
        .await
        .and_then(|response| match response {
            CoreResponse::Models { list } => Ok(list),
            other => Err(unexpected(&other)),
        });
    let course = match core
        .handle(CoreRequest::CreateCourse {
            topic: seed::TOPIC.to_owned(),
        })
        .await?
    {
        CoreResponse::Course { course } => course.id,
        other => return Err(unexpected(&other).into()),
    };

    let mut run = Run {
        core,
        out,
        selected,
        course,
        lines: Vec::new(),
    };
    run.all().await?;
    let lines = run.lines;

    // Nothing more may spend plan usage.
    core.observer.allow(&[]);
    let usage = core
        .handle(CoreRequest::GetUsage)
        .await
        .and_then(|response| match response {
            CoreResponse::Usage { usage } => Ok(usage),
            other => Err(unexpected(&other)),
        });
    let captured = core
        .capture
        .as_ref()
        .map(|capture| capture.write().map_err(|error| error.to_string()));
    Ok(Report {
        models,
        tasks: lines,
        usage,
        captured,
    })
}

fn unexpected(response: &CoreResponse) -> CoreError {
    CoreError::Provider {
        message: format!("the core answered unexpectedly: {response:?}"),
    }
}

/// A run under way on its course.
struct Run<'a> {
    core: &'a LiveCore,
    out: &'a mut dyn Write,
    selected: Vec<LiveTask>,
    course: Uuid,
    lines: Vec<TaskReport>,
}

impl Run<'_> {
    fn selected(&self, task: LiveTask) -> bool {
        self.selected.contains(&task)
    }

    /// Whether a selected task comes after `task`.
    fn needed_after(&self, task: LiveTask) -> bool {
        self.selected.iter().any(|selected| {
            LiveTask::ALL.iter().position(|t| t == selected)
                > LiveTask::ALL.iter().position(|t| *t == task)
        })
    }

    fn store(&self) -> &Store {
        &self.core.store
    }

    async fn all(&mut self) -> Result<(), CoreError> {
        let ladder = self.placement().await?;
        let index = if self.needed_after(LiveTask::PlacementQuestion) {
            self.index(ladder).await?
        } else {
            false
        };
        let section = if self.needed_after(LiveTask::BuildIndex) {
            self.sections(index).await?
        } else {
            None
        };
        if self.selected(LiveTask::ProposeDeepDives) {
            self.deep_dives(section).await?;
        }
        if self.selected(LiveTask::ExplainDifferently) {
            self.explain(section).await?;
        }
        if self.selected(LiveTask::Define) {
            self.define(section).await?;
        }
        if self.selected(LiveTask::SideChatReply) {
            self.side_chat(section).await?;
        }
        Ok(())
    }

    fn starting(&mut self, task: LiveTask) {
        let _ = writeln!(self.out, "{}…", task.name());
    }

    /// Reports `task` from the calls spent on it.
    fn report(&mut self, task: LiveTask, result: LiveResult, memory: Vec<Memory>) {
        let calls: Vec<Call> = self
            .core
            .observer
            .calls()
            .into_iter()
            .filter(|call| call.live == task)
            .collect();
        let own: Vec<&Call> = calls.iter().filter(|call| !call.is_repair()).collect();
        // A request made without a capability shows the route refused it;
        // only an answered request with it shows the route took it.
        let structured = if !task.structured() {
            None
        } else if own.iter().any(|call| !call.schema) {
            Some(Structured::Fallback)
        } else if own.iter().any(|call| call.answered()) {
            Some(Structured::Accepted)
        } else {
            None
        };
        let search = if !task.searches() {
            None
        } else if own.iter().any(|call| !call.web_search) {
            Some(Search::Refused)
        } else if own.iter().any(|call| call.answered()) {
            Some(Search::Allowed)
        } else {
            None
        };
        let result_word = match &result {
            LiveResult::Passed => "passed".to_owned(),
            LiveResult::Failed(error) => format!("failed: {error}"),
            LiveResult::Skipped => "skipped: a task it needs failed".to_owned(),
        };
        let _ = writeln!(self.out, "  {result_word}");
        self.lines.push(TaskReport {
            task,
            result,
            structured,
            search,
            model: own.first().and_then(|call| call.model.clone()),
            repairs: calls.iter().filter(|call| call.is_repair()).count() as u32,
            took: calls.iter().map(Call::took).sum(),
            memory,
        });
    }

    fn repaired(&self, task: LiveTask) -> Validity {
        let repaired = self
            .core
            .observer
            .calls()
            .iter()
            .any(|call| call.live == task && call.task == Task::RepairStructured);
        if repaired {
            Validity::AfterRepair
        } else {
            Validity::AtOnce
        }
    }

    /// Draft ladder and placement question, from the self-rating. Answers
    /// whether the course has its ladder.
    async fn placement(&mut self) -> Result<bool, CoreError> {
        let draft = self.selected(LiveTask::DraftLadder);
        let question = self.selected(LiveTask::PlacementQuestion);
        let course = self.course;
        if !draft {
            self.store().store_ladder(course, &seed::ladder())?;
        }
        if !draft && !question {
            self.store()
                .set_self_rating(course, seed::RATING, seed::GOAL)?;
            return Ok(true);
        }
        if draft {
            self.starting(LiveTask::DraftLadder);
        }
        let answer = self
            .core
            .handle(CoreRequest::SubmitSelfRating {
                course_id: course,
                rating: seed::RATING,
                goal: seed::GOAL,
            })
            .await;
        let (asked, error) = match answer {
            Ok(CoreResponse::Placement { placement, .. }) => (
                matches!(placement.step, PlacementStep::Question { .. }),
                placement.error,
            ),
            Ok(other) => (false, Some(unexpected(&other))),
            Err(error) => (false, Some(error)),
        };
        let failed = |error: &Option<CoreError>, what: &str| {
            LiveResult::Failed(error.clone().unwrap_or_else(|| CoreError::Provider {
                message: format!("no {what} was stored"),
            }))
        };
        let ladder = self.store().ladder(course)?.is_some();
        if draft {
            let result = if ladder {
                LiveResult::Passed
            } else {
                failed(&error, "ladder")
            };
            self.report(LiveTask::DraftLadder, result, Vec::new());
        }
        if question {
            self.starting(LiveTask::PlacementQuestion);
            let result = if !ladder {
                LiveResult::Skipped
            } else if asked {
                LiveResult::Passed
            } else {
                failed(&error, "question")
            };
            self.report(LiveTask::PlacementQuestion, result, Vec::new());
        }
        Ok(ladder)
    }

    /// Build index, once placement is finished with seeded answers, or a
    /// seeded index; skipped without a `ladder`. Answers whether the course
    /// has its index.
    async fn index(&mut self, ladder: bool) -> Result<bool, CoreError> {
        let course = self.course;
        let selected = self.selected(LiveTask::BuildIndex);
        if !ladder {
            if selected {
                self.starting(LiveTask::BuildIndex);
                self.report(LiveTask::BuildIndex, LiveResult::Skipped, Vec::new());
            }
            return Ok(false);
        }
        seed::finish_placement(self.store(), course)?;
        if !selected {
            seed::index(self.store(), course)?;
            return Ok(true);
        }
        self.starting(LiveTask::BuildIndex);
        let answer = self
            .core
            .handle(CoreRequest::RetryPlacementStep { course_id: course })
            .await;
        let error = match answer {
            Ok(CoreResponse::Placement { placement, .. }) => match placement.step {
                PlacementStep::Done => None,
                _ => Some(placement.error.unwrap_or_else(|| CoreError::Provider {
                    message: "placement did not finish".to_owned(),
                })),
            },
            Ok(other) => Some(unexpected(&other)),
            Err(error) => Some(error),
        };
        let built = error.is_none() && !self.store().index_tree(course)?.chapters.is_empty();
        let (result, memory) = match error {
            Some(error) => (LiveResult::Failed(error), Vec::new()),
            None => (
                LiveResult::Passed,
                vec![Memory::Map {
                    valid: self.repaired(LiveTask::BuildIndex),
                    concepts: self.store().course_map(course)?.concepts.len(),
                    conventions: self.store().conventions(course)?.len(),
                }],
            ),
        };
        self.report(LiveTask::BuildIndex, result, memory);
        Ok(built)
    }

    /// Write section on the first two sections, in order, or a seeded
    /// version of the first. Answers the first section once it is written.
    async fn sections(&mut self, index: bool) -> Result<Option<Uuid>, CoreError> {
        let selected = self.selected(LiveTask::WriteSection);
        if !index {
            if selected {
                self.starting(LiveTask::WriteSection);
                self.report(LiveTask::WriteSection, LiveResult::Skipped, Vec::new());
            }
            return Ok(None);
        }
        let tree = self.store().index_tree(self.course)?;
        let sections: Vec<Uuid> = in_index_order(&tree)
            .iter()
            .take(2)
            .map(|entry| entry.section.id)
            .collect();
        let Some(&first) = sections.first() else {
            return Err(CoreError::NotFound {
                what: "a section in the course's index".to_owned(),
            });
        };
        if !selected {
            seed::version(self.store(), first)?;
            return Ok(Some(first));
        }
        self.starting(LiveTask::WriteSection);
        let mut memory = Vec::new();
        for section in sections {
            let since = self.core.observer.calls().len();
            let answer = self
                .core
                .handle(CoreRequest::OpenSection {
                    course_id: self.course,
                    section_id: section,
                })
                .await;
            match answer {
                Ok(CoreResponse::Section { .. }) => {
                    memory.push(self.record(section, since)?);
                }
                Ok(other) => return self.fail_sections(unexpected(&other), memory),
                Err(error) => return self.fail_sections(error, memory),
            }
        }
        self.report(LiveTask::WriteSection, LiveResult::Passed, memory);
        Ok(Some(first))
    }

    fn fail_sections(
        &mut self,
        error: CoreError,
        memory: Vec<Memory>,
    ) -> Result<Option<Uuid>, CoreError> {
        self.report(LiveTask::WriteSection, LiveResult::Failed(error), memory);
        Ok(None)
    }

    /// The record the version just written for `section` stored, from the
    /// calls made since `since`.
    fn record(&self, section: Uuid, since: usize) -> Result<Memory, CoreError> {
        let calls = self.core.observer.calls();
        let calls = &calls[since.min(calls.len())..];
        let own = calls.iter().rev().find(|call| !call.is_repair());
        let repair = calls
            .iter()
            .rev()
            .find(|call| call.task == Task::RepairRecord && call.outcome == Outcome::Completed);
        let title = self
            .store()
            .index_node(section)?
            .map(|node| node.title)
            .unwrap_or_default();
        let stored = self
            .store()
            .current_version(section)?
            .and_then(|version| version.record);
        let (record, rejected) = match stored {
            Some(stored) if stored.status == RecordStatus::Confirmed => {
                let raw = match repair {
                    Some(repair) => record::validate(strip_fence(&repair.text), &[] as &[&str]),
                    None => match own.and_then(|call| record_payload(&call.text)) {
                        Some(payload) => record::validate(&payload, &[] as &[&str]),
                        None => Ok(stored.record.clone()),
                    },
                };
                let rejected = raw.map_or(0, |raw| {
                    fixed_entries(&raw).saturating_sub(fixed_entries(&stored.record))
                });
                (RecordState::Confirmed, rejected)
            }
            _ => (RecordState::Unconfirmed, 0),
        };
        Ok(Memory::Record {
            section: title,
            record,
            rejected,
            input_chars: own.map_or(0, |call| call.input_chars),
        })
    }

    async fn deep_dives(&mut self, section: Option<Uuid>) -> Result<(), CoreError> {
        let task = LiveTask::ProposeDeepDives;
        self.starting(task);
        let Some(section) = section else {
            self.report(task, LiveResult::Skipped, Vec::new());
            return Ok(());
        };
        let answer = self
            .core
            .handle(CoreRequest::GoDeeper {
                course_id: self.course,
                section_id: section,
            })
            .await;
        match answer {
            Ok(CoreResponse::DeepDives { .. }) => {
                let valid = self.repaired(task);
                self.report(task, LiveResult::Passed, vec![Memory::Extension { valid }]);
            }
            Ok(other) => self.report(task, LiveResult::Failed(unexpected(&other)), Vec::new()),
            Err(error) => self.report(task, LiveResult::Failed(error), Vec::new()),
        }
        Ok(())
    }

    async fn explain(&mut self, section: Option<Uuid>) -> Result<(), CoreError> {
        let task = LiveTask::ExplainDifferently;
        self.starting(task);
        let Some(section) = section else {
            self.report(task, LiveResult::Skipped, Vec::new());
            return Ok(());
        };
        let since = self.core.observer.calls().len();
        let answer = self
            .core
            .handle(CoreRequest::ExplainDifferently {
                course_id: self.course,
                section_id: section,
                style: ExplainStyle::Simpler,
            })
            .await;
        match answer {
            Ok(CoreResponse::Section { .. }) => {
                let memory = vec![self.record(section, since)?];
                self.report(task, LiveResult::Passed, memory);
            }
            Ok(other) => self.report(task, LiveResult::Failed(unexpected(&other)), Vec::new()),
            Err(error) => self.report(task, LiveResult::Failed(error), Vec::new()),
        }
        Ok(())
    }

    async fn define(&mut self, section: Option<Uuid>) -> Result<(), CoreError> {
        let task = LiveTask::Define;
        self.starting(task);
        let Some((anchor, sentence)) = self.passage(section)? else {
            self.report(task, LiveResult::Skipped, Vec::new());
            return Ok(());
        };
        let term = longest_word(&sentence);
        let anchor = Anchor {
            quote: term.to_owned(),
            ..anchor
        };
        let answer = self
            .core
            .handle(CoreRequest::Define { anchor, sentence })
            .await;
        let result = match answer {
            Ok(CoreResponse::Definition { .. }) => LiveResult::Passed,
            Ok(other) => LiveResult::Failed(unexpected(&other)),
            Err(error) => LiveResult::Failed(error),
        };
        self.report(task, result, Vec::new());
        Ok(())
    }

    async fn side_chat(&mut self, section: Option<Uuid>) -> Result<(), CoreError> {
        let task = LiveTask::SideChatReply;
        self.starting(task);
        let Some((anchor, _)) = self.passage(section)? else {
            self.report(task, LiveResult::Skipped, Vec::new());
            return Ok(());
        };
        let result = match self.chat(anchor).await {
            Ok(()) => LiveResult::Passed,
            Err(error) => LiveResult::Failed(error),
        };
        self.report(task, result, Vec::new());
        Ok(())
    }

    async fn chat(&self, anchor: Anchor) -> Result<(), CoreError> {
        let chat_id = match self
            .core
            .handle(CoreRequest::StartSideChat {
                course_id: self.course,
                anchor: Some(anchor),
            })
            .await?
        {
            CoreResponse::SideChat { chat } => chat.id,
            other => return Err(unexpected(&other)),
        };
        match self
            .core
            .handle(CoreRequest::SendSideChatMessage {
                chat_id,
                text: "Could you explain this passage again with an everyday example?".to_owned(),
            })
            .await?
        {
            CoreResponse::SideChatReply { .. } => Ok(()),
            other => Err(unexpected(&other)),
        }
    }

    /// The first sentence of the first prose block of the section's current
    /// version, anchored there; none when the section was not written.
    fn passage(&self, section: Option<Uuid>) -> Result<Option<(Anchor, String)>, CoreError> {
        let Some(section) = section else {
            return Ok(None);
        };
        let Some(version) = self.store().current_version(section)? else {
            return Ok(None);
        };
        let prose = version
            .blocks
            .iter()
            .find_map(|block| match &block.content {
                BlockContent::Prose { text } if !text.trim().is_empty() => {
                    Some((block.index, text))
                }
                _ => None,
            });
        let Some((block, text)) = prose else {
            return Ok(None);
        };
        let sentence = first_sentence(text).to_owned();
        Ok(Some((
            Anchor {
                version_id: version.id,
                block,
                quote: sentence.clone(),
            },
            sentence,
        )))
    }
}

/// The payload of the record block a section's text closes with.
fn record_payload(text: &str) -> Option<String> {
    let mut splitter = Splitter::new();
    let mut events = splitter.push(text.as_bytes());
    events.extend(splitter.finish());
    events.into_iter().find_map(|event| match event {
        SplitEvent::Closed {
            kind: ReservedKind::Record,
            payload,
            ..
        }
        | SplitEvent::Unclosed {
            kind: ReservedKind::Record,
            payload,
            ..
        } => Some(payload),
        _ => None,
    })
}

/// The answer without a Markdown code fence around it.
fn strip_fence(answer: &str) -> &str {
    let answer = answer.trim();
    answer
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
        .map(|inner| inner.strip_prefix("json").unwrap_or(inner).trim())
        .unwrap_or(answer)
}

/// The notation and term entries of a record.
fn fixed_entries(record: &SectionRecord) -> usize {
    record.notation.len() + record.terms.len()
}

fn first_sentence(text: &str) -> &str {
    let text = text.trim();
    let end = text
        .char_indices()
        .find(|(_, c)| matches!(c, '.' | '!' | '?' | '\n'))
        .map_or(text.len(), |(at, c)| at + c.len_utf8());
    text[..end].trim()
}

/// The longest word of `sentence`, without its punctuation: a term worth
/// defining, quoted exactly as it stands.
fn longest_word(sentence: &str) -> &str {
    sentence
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .max_by_key(|word| word.chars().count())
        .filter(|word| !word.is_empty())
        .unwrap_or(sentence)
}
