//! Starting a course and placing the candidate on its concept ladder before
//! the index is built (ADR-0006).
//!
//! For now `CreateCourse` keeps the thin loop: it builds the index directly.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, CourseView, Goal, PlacementAnswer, PlacementState, SelfRating};
use crate::store::Store;

const FEATURE: &str = "placement";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn create_course(
        &self,
        cx: &Features,
        topic: &str,
    ) -> Result<CourseView, CoreError> {
        let topic = topic.trim();
        if topic.is_empty() {
            return Err(CoreError::InvalidRequest {
                message: "the topic is empty".to_owned(),
            });
        }
        cx.index.build_from_topic(cx, topic).await
    }

    pub(crate) async fn submit_self_rating(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _rating: SelfRating,
        _goal: Goal,
    ) -> Result<PlacementState, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn answer(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _question_id: Uuid,
        _answer: PlacementAnswer,
    ) -> Result<PlacementState, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn retry_step(
        &self,
        _cx: &Features,
        _course_id: Uuid,
    ) -> Result<PlacementState, CoreError> {
        Err(unavailable(FEATURE))
    }
}
