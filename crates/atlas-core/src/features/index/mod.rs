//! Building a course's index (the broad pass) and its course map.
//!
//! For now a thin build: the broad pass from the topic, the profile and the
//! goal, with no course map.

use std::fmt::Write as _;
use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::domain::course_map::CourseMap;
use crate::domain::profile::Knowledge;
use crate::harness::index::IndexDraft;
use crate::harness::prompts;
use crate::index_tree::{IndexTree, TreeChapter, TreeSection};
use crate::protocol::{CoreError, Goal};
use crate::provider::{ModelProvider, ProviderRequest, Task, complete_text};
use crate::store::Store;

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
        })
    }

    /// Builds and stores the index of a course from its topic, its ladder,
    /// the candidate profile and the goal, and moves the course to the ready
    /// stage.
    pub(crate) async fn build(&self, cx: &Features, course_id: Uuid) -> Result<(), CoreError> {
        let course = self
            .store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })?;
        let text = complete_text(
            self.provider.as_ref(),
            ProviderRequest {
                model: cx.models.current(cx).await?,
                ..ProviderRequest::new(
                    Task::BuildIndex,
                    prompts::BUILD_INDEX.text(),
                    self.input(course_id, &course.topic)?,
                )
            },
            |_| {},
        )
        .await?;
        let index: IndexDraft =
            serde_json::from_str(&text).map_err(|error| CoreError::Provider {
                message: format!("the index is not valid: {error}"),
            })?;
        if index.chapters.len() < 2 || index.chapters.iter().any(|c| c.sections.is_empty()) {
            return Err(CoreError::Provider {
                message: "the index needs at least two chapters, each with sections".to_owned(),
            });
        }
        let map = CourseMap {
            concepts: Vec::new(),
            entries: Vec::new(),
        };
        self.store
            .insert_index(course_id, &tree(index), &map, &[])?;
        Ok(())
    }

    /// The topic, the goal, each rung with its state and the misconceptions
    /// picked.
    fn input(&self, course_id: Uuid, topic: &str) -> Result<String, CoreError> {
        let mut input = format!("Topic: {topic}\n");
        let Some(profile) = self.store.profile(course_id)? else {
            return Ok(input);
        };
        let goal = match profile.goal {
            Goal::Overview => "overview",
            Goal::WorkingKnowledge => "working knowledge",
            Goal::Expert => "expert",
        };
        let _ = writeln!(input, "Goal: {goal}\n\nLadder, most basic first:");
        let ladder = self.store.ladder(course_id)?;
        for rung in &profile.rungs {
            let concept = ladder
                .iter()
                .flat_map(|ladder| &ladder.rungs)
                .find(|candidate| candidate.id == rung.rung)
                .map_or("", |candidate| candidate.concept.as_str());
            let state = match rung.state {
                Knowledge::Known => "known",
                Knowledge::Shaky => "shaky",
                Knowledge::Unknown => "unknown",
            };
            let _ = writeln!(input, "- rung {} ({state}): {concept}", rung.rung.0);
        }
        if !profile.misconceptions.is_empty() {
            input.push_str("\nMisconceptions picked:\n");
            for misconception in &profile.misconceptions {
                let _ = writeln!(
                    input,
                    "- rung {}: {}",
                    misconception.rung.0, misconception.text
                );
            }
        }
        Ok(input)
    }
}

/// The drafted broad pass as a tree with new ids, nothing read.
fn tree(index: IndexDraft) -> IndexTree {
    IndexTree {
        chapters: index
            .chapters
            .into_iter()
            .map(|chapter| TreeChapter {
                id: Uuid::new_v4(),
                title: chapter.title,
                sections: chapter
                    .sections
                    .into_iter()
                    .map(|section| TreeSection {
                        id: Uuid::new_v4(),
                        title: section.title,
                        line: section.summary,
                        read: false,
                        children: Vec::new(),
                    })
                    .collect(),
            })
            .collect(),
    }
}
