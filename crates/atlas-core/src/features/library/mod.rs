//! The library: courses most recently opened first, opening one where it
//! resumes, and permanent deletion.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, CourseSummary, CourseView};
use crate::store::Store;

const FEATURE: &str = "library";

pub(crate) struct Feature {
    store: Arc<Store>,
}

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
        })
    }

    pub(crate) async fn list_courses(
        &self,
        _cx: &Features,
    ) -> Result<Vec<CourseSummary>, CoreError> {
        Ok(self.store.list_courses()?)
    }

    pub(crate) async fn open_course(
        &self,
        _cx: &Features,
        course_id: Uuid,
    ) -> Result<CourseView, CoreError> {
        self.course(course_id)
    }

    pub(crate) async fn delete_course(
        &self,
        _cx: &Features,
        _course_id: Uuid,
    ) -> Result<(), CoreError> {
        Err(unavailable(FEATURE))
    }

    /// A course with its index, without recording that it was opened.
    pub(crate) fn course(&self, course_id: Uuid) -> Result<CourseView, CoreError> {
        self.store
            .course(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })
    }
}
