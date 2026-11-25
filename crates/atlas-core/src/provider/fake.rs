use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use futures::stream;
use serde_json::{Value, json};

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
    /// Scripted outcomes that answer their task, one per request, in order.
    answers: Mutex<Vec<(Task, Answers)>>,
    requests: Mutex<Vec<ProviderRequest>>,
}

/// Scripted outcomes for one task: a completed response's text, or an error.
type Answers = VecDeque<Result<String, ProviderError>>;

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
            answers: Mutex::new(Vec::new()),
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

    /// Answers `task` with these outcomes, one per request in order, instead
    /// of the script: the text of a completed response, or the error the call
    /// fails with. A request after the last outcome fails.
    pub fn answer(
        self,
        task: Task,
        answers: impl IntoIterator<Item = Result<String, ProviderError>>,
    ) -> Self {
        {
            let mut scripted = self.answers.lock().expect("fake provider lock poisoned");
            match scripted.iter_mut().find(|(scripted, _)| *scripted == task) {
                Some((_, queue)) => queue.extend(answers),
                None => scripted.push((task, answers.into_iter().collect())),
            }
        }
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

    /// The next scripted outcome for `task`, if its answers are scripted.
    fn next_answer(&self, task: Task) -> Option<Result<String, ProviderError>> {
        let mut scripted = self.answers.lock().expect("fake provider lock poisoned");
        let (_, queue) = scripted
            .iter_mut()
            .find(|(scripted, _)| *scripted == task)?;
        Some(queue.pop_front().unwrap_or_else(|| {
            Err(ProviderError::failed(format!(
                "the fake provider has no more answers for {task:?}"
            )))
        }))
    }
}

#[async_trait]
impl ModelProvider for FakeProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        if let Some((_, recording)) = self.replays.iter().find(|(task, _)| *task == request.task) {
            self.record(request);
            return Ok(Box::pin(stream::iter(replay(recording))));
        }
        if let Some(answer) = self.next_answer(request.task) {
            self.record(request);
            let events = chunk(&answer?, 24)
                .into_iter()
                .map(|delta| Ok(ProviderEvent::TextDelta(delta)))
                .chain([Ok(ProviderEvent::Completed)]);
            return Ok(Box::pin(stream::iter(events)));
        }
        let deltas = match request.task {
            Task::BuildIndex => chunk(&index_answer(&self.script.index).to_string(), 24),
            Task::DraftLadder => chunk(SAMPLE_LADDER, 24),
            Task::PlacementQuestion => chunk(SAMPLE_QUESTION, 24),
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

/// Answers every draft-ladder request: eight generic concepts.
const SAMPLE_LADDER: &str = r#"{"concepts": ["The vocabulary", "The basic objects", "How the objects combine", "The core rule", "Applying the core rule", "The common exceptions", "Trade-offs between approaches", "Open problems"]}"#;

/// Answers every placement-question request.
const SAMPLE_QUESTION: &str = r#"{"stem": "You have applied the core rule once. What do you do next?", "options": [{"text": "Check the result against the basic objects", "misconception": null}, {"text": "Apply the rule again until it stops changing", "misconception": "The rule converges on its own"}, {"text": "Start over with a different rule", "misconception": "One application proves the rule wrong"}, {"text": "Stop, the result is final", "misconception": "A rule applied once needs no check"}], "key": 0}"#;

/// The build-index answer for `index`, with every field the schema requires:
/// sections that introduce and rely on nothing, and no concepts or
/// conventions.
fn index_answer(index: &IndexDraft) -> Value {
    let chapters: Vec<Value> = index
        .chapters
        .iter()
        .map(|chapter| {
            let sections: Vec<Value> = chapter
                .sections
                .iter()
                .map(|section| {
                    json!({
                        "title": section.title,
                        "summary": section.summary,
                        "introduces": [],
                        "relies_on": []
                    })
                })
                .collect();
            json!({ "title": chapter.title, "sections": sections })
        })
        .collect();
    json!({ "chapters": chapters, "concepts": [], "conventions": [] })
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
