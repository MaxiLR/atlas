//! The raw streams the plan route sent, kept as fixture candidates: one
//! `.sse` file per streamed response, named by its number and its task.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::provider::Task;
use crate::provider::chatgpt::StreamTap;

/// Collects each streamed body in memory as it arrives, so the stream is
/// never held up, and writes them under its folder when asked.
pub struct Capture {
    dir: PathBuf,
    streams: Mutex<BTreeMap<u64, (Task, Vec<u8>)>>,
}

impl Capture {
    /// A capture written under `dir`, created when written.
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
            streams: Mutex::default(),
        }
    }

    /// Writes every stream received so far, `<number>-<task>.sse`, and
    /// answers the files written.
    pub fn write(&self) -> std::io::Result<Vec<PathBuf>> {
        let streams = self
            .streams
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        std::fs::create_dir_all(&self.dir)?;
        streams
            .into_iter()
            .map(|(number, (task, bytes))| {
                let path = self
                    .dir
                    .join(format!("{number:02}-{}.sse", task_name(task)));
                std::fs::write(&path, bytes)?;
                Ok(path)
            })
            .collect()
    }
}

impl StreamTap for Capture {
    fn chunk(&self, response: u64, task: Task, bytes: &[u8]) {
        self.streams
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(response)
            .or_insert_with(|| (task, Vec::new()))
            .1
            .extend_from_slice(bytes);
    }
}

fn task_name(task: Task) -> &'static str {
    match task {
        Task::DraftLadder => "draft-ladder",
        Task::PlacementQuestion => "placement-question",
        Task::BuildIndex => "build-index",
        Task::WriteSection => "write-section",
        Task::ProposeDeepDives => "propose-deep-dives",
        Task::ExplainDifferently => "explain-differently",
        Task::Define => "define",
        Task::SideChatReply => "side-chat-reply",
        Task::RepairStructured => "repair-structured",
        Task::RepairBlock => "repair-block",
        Task::RepairRecord => "repair-record",
    }
}
