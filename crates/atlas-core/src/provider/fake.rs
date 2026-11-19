use std::sync::Mutex;

use async_trait::async_trait;
use futures::stream;

use super::chatgpt::ResponsesStreamParser;
use super::{ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, Task};
use crate::harness::index::{ChapterDraft, IndexDraft, SectionDraft};
use crate::protocol::Model;

/// What a [`FakeProvider`] answers, per task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeScript {
    /// Returned, as JSON, for every build-index request.
    pub index: IndexDraft,
    /// Streamed, in order, for every write-section request.
    pub section_deltas: Vec<String>,
}

impl FakeScript {
    /// A small generic course: three chapters of two sections each.
    pub fn sample() -> Self {
        let chapter = |title: &str, sections: [(&str, &str); 2]| ChapterDraft {
            title: title.to_owned(),
            sections: sections
                .into_iter()
                .map(|(title, summary)| SectionDraft {
                    title: title.to_owned(),
                    summary: summary.to_owned(),
                })
                .collect(),
        };
        Self {
            index: IndexDraft {
                chapters: vec![
                    chapter(
                        "Foundations",
                        [
                            ("What it is", "The territory in one page."),
                            ("Why it matters", "Where the topic shows up."),
                        ],
                    ),
                    chapter(
                        "Core ideas",
                        [
                            ("The first principle", "The idea everything rests on."),
                            ("The second principle", "How the first one is used."),
                        ],
                    ),
                    chapter(
                        "Going further",
                        [
                            ("Open questions", "What is still debated."),
                            ("Beyond the basics", "The deep dives worth taking."),
                        ],
                    ),
                ],
            },
            section_deltas: [
                "This section opens with the main idea. ",
                "It then builds on it step by step. ",
                "It closes with what comes next.",
            ]
            .map(str::to_owned)
            .to_vec(),
        }
    }
}

/// A scripted [`ModelProvider`] that records every request it receives.
#[derive(Debug)]
pub struct FakeProvider {
    script: FakeScript,
    models: Vec<Model>,
    /// Recorded Responses streams that answer their task instead of the script.
    replays: Vec<(Task, Vec<u8>)>,
    requests: Mutex<Vec<ProviderRequest>>,
}

impl FakeProvider {
    /// Lists one model, `fake-model`.
    pub fn new(script: FakeScript) -> Self {
        Self {
            script,
            models: vec![Model {
                slug: "fake-model".to_owned(),
                display_name: "Fake model".to_owned(),
            }],
            replays: Vec::new(),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Lists these models instead, in this order.
    pub fn with_models(mut self, models: Vec<Model>) -> Self {
        self.models = models;
        self
    }

    /// Answers `task` by replaying this recorded Responses stream (the bytes
    /// of an `.sse` file) through the Responses stream parser.
    pub fn replay(mut self, task: Task, recording: impl Into<Vec<u8>>) -> Self {
        self.replays.push((task, recording.into()));
        self
    }

    /// The requests received so far, in order.
    pub fn requests(&self) -> Vec<ProviderRequest> {
        self.requests
            .lock()
            .expect("fake provider lock poisoned")
            .clone()
    }

    fn record(&self, request: ProviderRequest) {
        self.requests
            .lock()
            .expect("fake provider lock poisoned")
            .push(request);
    }
}

#[async_trait]
impl ModelProvider for FakeProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        if let Some((_, recording)) = self.replays.iter().find(|(task, _)| *task == request.task) {
            self.record(request);
            return Ok(Box::pin(stream::iter(replay(recording))));
        }
        let deltas = match request.task {
            Task::BuildIndex => {
                let json = serde_json::to_string(&self.script.index)
                    .map_err(|error| ProviderError::failed(error.to_string()))?;
                chunk(&json, 24)
            }
            Task::WriteSection => self.script.section_deltas.clone(),
            task => {
                return Err(ProviderError::failed(format!(
                    "the fake provider has no script for {task:?}"
                )));
            }
        };
        self.record(request);
        let events = deltas
            .into_iter()
            .map(|delta| Ok(ProviderEvent::TextDelta(delta)))
            .chain([Ok(ProviderEvent::Completed)]);
        Ok(Box::pin(stream::iter(events)))
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        Ok(self.models.clone())
    }
}

fn chunk(text: &str, size: usize) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(size)
        .map(|chars| chars.iter().collect())
        .collect()
}

/// The recording parsed as it would arrive: in small chunks, cut anywhere.
fn replay(recording: &[u8]) -> Vec<Result<ProviderEvent, ProviderError>> {
    let mut parser = ResponsesStreamParser::new(Some("req_fake_replay".to_owned()));
    let mut events = Vec::new();
    for chunk in recording.chunks(7) {
        events.extend(parser.feed(chunk));
    }
    events.extend(parser.finish());
    events
        .into_iter()
        .map(|event| event.into_provider())
        .collect()
}
