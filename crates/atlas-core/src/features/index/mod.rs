//! Building a course's index (the broad pass) and its course map (ADR-0004,
//! ADR-0013): one structured answer holds both, and the map never leaves the
//! core.

mod task;

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::domain::ladder::Ladder;
use crate::harness::structured::{StructuredError, StructuredRunner};
use crate::ports::EventSink;
use crate::protocol::{CoreError, CoreEvent, CourseStage, Goal};
use crate::store::Store;

pub(crate) struct Feature {
    store: Arc<Store>,
    events: Arc<dyn EventSink>,
    runner: StructuredRunner,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            events: deps.events.clone(),
            runner: StructuredRunner::new(deps.provider.clone()),
        })
    }

    /// Builds the index of a course from its topic, its ladder, the candidate
    /// profile and the goal, and stores it with its course map and the
    /// conventions the plan fixed, in one transaction that moves the course
    /// to the ready stage; it then resumes at its first section. Only the
    /// tree is announced. A course without a profile is built as if every
    /// rung were unknown. A course that has an index already is only moved to
    /// the ready stage, with no request.
    pub(crate) async fn build(&self, cx: &Features, course_id: Uuid) -> Result<(), CoreError> {
        let course = self
            .store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })?;
        if !self.store.index_tree(course_id)?.chapters.is_empty() {
            if course.stage != CourseStage::Ready {
                self.store.set_stage(course_id, CourseStage::Ready)?;
            }
            return Ok(());
        }

        let ladder = self
            .store
            .ladder(course_id)?
            .unwrap_or(Ladder { rungs: Vec::new() });
        let profile = self.store.profile(course_id)?;
        let goal = profile
            .as_ref()
            .map(|profile| profile.goal)
            .or(course.goal)
            .unwrap_or(Goal::Overview);
        let input = task::input(&course.topic, goal, &ladder, profile.as_ref());
        let model = cx.models.current(cx).await?;
        let answer = self
            .runner
            .run(
                &task::BuildIndex { ladder: &ladder },
                model.as_deref(),
                &input,
            )
            .await
            .map_err(|error| match error {
                StructuredError::Provider(error) => error.into(),
                // The reasons name concepts of the map, so they stay here.
                StructuredError::Invalid { .. } => CoreError::Provider {
                    message: "the model's index did not hold, even after a repair".to_owned(),
                },
            })?;

        let plan = answer.plan();
        self.store
            .insert_index(course_id, &plan.tree, &plan.map, &plan.conventions)?;
        self.events.emit(CoreEvent::IndexChanged {
            course_id,
            chapters: cx.library.course(course_id)?.chapters,
        });
        Ok(())
    }
}
