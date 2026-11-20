//! The live command: each harness task run once against the real plan, on a
//! throwaway course, with a report for the engineer.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::CoreDeps;
use crate::protocol::{CoreError, ModelList, UsageState};
use crate::provider::Task;
use crate::provider::chatgpt::StreamTap;

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
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveResult {
    Passed,
    Failed(CoreError),
    /// A task it depends on failed.
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Structured {
    Accepted,
    Fallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Search {
    Allowed,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    AtOnce,
    AfterRepair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordState {
    Confirmed,
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Memory {
    Map {
        valid: Validity,
        concepts: usize,
        conventions: usize,
    },
    Record {
        section: String,
        record: RecordState,
        rejected: usize,
        input_chars: usize,
    },
    Extension {
        valid: Validity,
    },
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReport {
    pub task: LiveTask,
    pub result: LiveResult,
    pub structured: Option<Structured>,
    pub search: Option<Search>,
    pub model: Option<String>,
    pub repairs: u32,
    pub took: Duration,
    pub memory: Vec<Memory>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub models: Result<ModelList, CoreError>,
    pub tasks: Vec<TaskReport>,
    pub usage: Result<UsageState, CoreError>,
    pub captured: Option<Result<Vec<PathBuf>, String>>,
}

impl Report {
    pub fn failed(&self) -> bool {
        todo!()
    }

    pub fn table(&self) -> String {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LiveError {
    #[error("sign in with ChatGPT in the Atlas app first")]
    SignInFirst,
    #[error(transparent)]
    Core(#[from] CoreError),
}

pub struct LiveCore {}

impl LiveCore {
    pub fn new(_deps: CoreDeps) -> Result<Self, CoreError> {
        todo!()
    }

    pub fn with_capture(self, _capture: Arc<Capture>) -> Self {
        todo!()
    }
}

pub async fn run(
    _tasks: &[LiveTask],
    _core: &LiveCore,
    _out: &mut dyn Write,
) -> Result<Report, LiveError> {
    todo!()
}

pub struct Capture {}

impl Capture {
    pub fn new(_dir: &Path) -> Self {
        todo!()
    }

    pub fn write(&self) -> std::io::Result<Vec<PathBuf>> {
        todo!()
    }
}

impl StreamTap for Capture {
    fn chunk(&self, _response: u64, _task: Task, _bytes: &[u8]) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub tasks: Vec<LiveTask>,
    pub model: Option<String>,
    pub capture: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ArgsError(pub String);

pub fn parse_args(_args: impl IntoIterator<Item = String>) -> Result<Args, ArgsError> {
    todo!()
}
