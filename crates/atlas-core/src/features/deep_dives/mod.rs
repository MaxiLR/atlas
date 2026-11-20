//! "Go deeper": three to six child sections under a section, extending the
//! course map (ADR-0004, ADR-0013). One structured answer proposes the
//! children with their part of the map; the map never leaves the core.
//!
//! A child is opened like any section: the sections feature writes it from
//! its own map entry and its parent's record, and it can be deepened in turn.

mod task;

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::harness::structured::{StructuredError, StructuredRunner};
use crate::index_tree::in_index_order;
use crate::ports::EventSink;
use crate::protocol::{ChapterNode, CoreError, CoreEvent, Goal, SectionNode};
use crate::store::Store;
use task::{Course, ProposeDeepDives};

pub(crate) struct Feature {
    store: Arc<Store>,
    events: Arc<dyn EventSink>,
    runner: StructuredRunner,
    /// Held while a deep dive is proposed and stored, so a second request
    /// for the same section waits and then finds its children.
    proposing: tokio::sync::Mutex<()>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            events: deps.events.clone(),
            runner: StructuredRunner::new(deps.provider.clone()),
            proposing: tokio::sync::Mutex::new(()),
        })
    }

    /// The deep dives under a section, in order. A section that has them
    /// already answers them with no request. Otherwise a section with a
    /// stored version gets three to six children proposed from its record,
    /// its map entry, the concepts the course introduces, the conventions,
    /// the index titles and the profile; they are stored under it in order
    /// with their map entries and new concepts in one transaction, and only
    /// the tree is announced. A section never written is refused.
    pub(crate) async fn go_deeper(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<Vec<SectionNode>, CoreError> {
        let _proposing = self.proposing.lock().await;
        let course = self
            .store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })?;
        let tree = self.store.index_tree(course_id)?;
        let parent = in_index_order(&tree)
            .into_iter()
            .find(|entry| entry.section.id == section_id)
            .map(|entry| entry.section)
            .ok_or_else(|| CoreError::NotFound {
                what: format!("section {section_id} in course {course_id}"),
            })?;
        if !parent.children.is_empty() {
            return Ok(deep_dives(
                &cx.library.course(course_id)?.chapters,
                section_id,
            ));
        }
        let version =
            self.store
                .current_version(section_id)?
                .ok_or_else(|| CoreError::InvalidRequest {
                    message: format!(
                        "section {section_id} has not been written yet; a section is deepened once \
                     it has been read"
                    ),
                })?;

        let ladder = self
            .store
            .ladder(course_id)?
            .unwrap_or(Ladder { rungs: Vec::new() });
        // A course placed without a profile is deepened for a candidate who
        // knows no rung.
        let profile = self
            .store
            .profile(course_id)?
            .unwrap_or_else(|| CandidateProfile {
                goal: course.goal.unwrap_or(Goal::Overview),
                rungs: Vec::new(),
                misconceptions: Vec::new(),
            });
        let map = self.store.course_map(course_id)?;
        let conventions = self.store.conventions(course_id)?;
        let input = task::input(
            &Course {
                ladder: &ladder,
                profile: &profile,
                tree: &tree,
                map: &map,
                conventions: &conventions,
            },
            parent,
            version.record.as_ref(),
        );
        let task = ProposeDeepDives {
            parent: section_id,
            tree: &tree,
            map: &map,
            ladder: &ladder,
        };
        let model = cx.models.current(cx).await?;
        let answer = self
            .runner
            .run(&task, model.as_deref(), &input)
            .await
            .map_err(|error| match error {
                StructuredError::Provider(error) => error.into(),
                // The reasons name concepts of the map, so they stay here.
                StructuredError::Invalid { .. } => not_held(),
            })?;

        let proposals = answer.proposals();
        let extended = task.extend(&proposals).map_err(|_| not_held())?;
        self.store.insert_children(
            course_id,
            section_id,
            &proposals.children,
            &extended.concepts[map.concepts.len()..],
            &proposals.map.entries,
        )?;
        let chapters = cx.library.course(course_id)?.chapters;
        let children = deep_dives(&chapters, section_id);
        self.events.emit(CoreEvent::IndexChanged {
            course_id,
            chapters,
        });
        Ok(children)
    }
}

fn not_held() -> CoreError {
    CoreError::Provider {
        message: "the model's deep dive did not hold, even after a repair".to_owned(),
    }
}

/// The deep dives of the section `id` as the index draws them.
fn deep_dives(chapters: &[ChapterNode], id: Uuid) -> Vec<SectionNode> {
    fn find(nodes: &[SectionNode], id: Uuid) -> Option<&SectionNode> {
        nodes.iter().find_map(|node| {
            if node.id == id {
                Some(node)
            } else {
                find(&node.deep_dives, id)
            }
        })
    }
    chapters
        .iter()
        .find_map(|chapter| find(&chapter.sections, id))
        .map(|node| node.deep_dives.clone())
        .unwrap_or_default()
}
