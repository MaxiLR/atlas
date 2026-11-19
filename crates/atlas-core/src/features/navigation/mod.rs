//! Next: the section that follows in the course, by the index tree's rules.
//!
//! For now the thin loop: the following section of the broad pass, opened.

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::protocol::{CoreError, CoreResponse};
use crate::store::Store;

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn next(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<CoreResponse, CoreError> {
        let course = cx.library.course(course_id)?;
        let mut sections = course
            .chapters
            .iter()
            .flat_map(|chapter| &chapter.sections)
            .skip_while(|section| section.id != section_id);
        if sections.next().is_none() {
            return Err(CoreError::NotFound {
                what: format!("section {section_id}"),
            });
        }
        match sections.next() {
            Some(following) => Ok(CoreResponse::Section {
                section: cx.sections.open(cx, course_id, following.id).await?,
            }),
            None => Ok(CoreResponse::EndOfCourse { course_id }),
        }
    }
}
