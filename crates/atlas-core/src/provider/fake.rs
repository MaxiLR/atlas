use std::sync::Mutex;

use async_trait::async_trait;
use futures::stream;

use super::{ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, Task};
use crate::harness::index::{ChapterDraft, IndexDraft, SectionDraft};

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
    requests: Mutex<Vec<ProviderRequest>>,
}

impl FakeProvider {
    pub fn new(script: FakeScript) -> Self {
        Self {
            script,
            requests: Mutex::new(Vec::new()),
        }
    }

    /// The requests received so far, in order.
    pub fn requests(&self) -> Vec<ProviderRequest> {
        self.requests
            .lock()
            .expect("fake provider lock poisoned")
            .clone()
    }
}

#[async_trait]
impl ModelProvider for FakeProvider {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let deltas = match request.task {
            Task::BuildIndex => {
                let json = serde_json::to_string(&self.script.index)
                    .map_err(|error| ProviderError::Failed(error.to_string()))?;
                chunk(&json, 24)
            }
            Task::WriteSection => self.script.section_deltas.clone(),
        };
        self.requests
            .lock()
            .expect("fake provider lock poisoned")
            .push(request);
        let events = deltas
            .into_iter()
            .map(|delta| Ok(ProviderEvent::TextDelta(delta)))
            .chain([Ok(ProviderEvent::Completed)]);
        Ok(Box::pin(stream::iter(events)))
    }
}

fn chunk(text: &str, size: usize) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(size)
        .map(|chars| chars.iter().collect())
        .collect()
}
