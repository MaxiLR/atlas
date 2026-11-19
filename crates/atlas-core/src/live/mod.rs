//! The live command: each harness task run once against the real plan, on a
//! throwaway course, with a report for the engineer.

use std::time::Duration;

use crate::protocol::CoreError;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveResult {
    Passed,
    Failed(CoreError),
    /// A task it depends on failed.
    Skipped,
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReport {
    pub task: LiveTask,
    pub result: LiveResult,
    pub model: Option<String>,
    pub repairs: u32,
    pub took: Duration,
}
