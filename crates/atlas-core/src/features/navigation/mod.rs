//! Next: where the course goes from a node, by the index tree's rules. It
//! only answers; the interface then opens the section.

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::index_tree;
use crate::protocol::{CoreError, CoreResponse};
use crate::store::Store;

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    /// The section that follows `node` (a section at any depth, or a
    /// chapter), or the end of the course after its last section.
    pub(crate) async fn next(
        &self,
        cx: &Features,
        course_id: Uuid,
        node: Uuid,
    ) -> Result<CoreResponse, CoreError> {
        let tree = cx.library.tree(course_id)?;
        let in_index = tree.chapters.iter().any(|chapter| chapter.id == node)
            || index_tree::state(&tree, node).is_some();
        if !in_index {
            return Err(CoreError::NotFound {
                what: format!("section {node} in course {course_id}"),
            });
        }
        Ok(match index_tree::next(&tree, node) {
            Some(section_id) => CoreResponse::NextSection {
                course_id,
                section_id,
            },
            None => CoreResponse::EndOfCourse { course_id },
        })
    }
}
