//! The library: courses most recently opened first, opening one where it
//! resumes, and permanent deletion.

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::index_tree::{self, IndexTree, TreeSection};
use crate::ports::EventSink;
use crate::protocol::{
    ChapterNode, CoreError, CoreEvent, CourseSummary, CourseView, SectionKind, SectionNode,
};
use crate::store::{CourseRecord, Store};

pub(crate) struct Feature {
    store: Arc<Store>,
    events: Arc<dyn EventSink>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            events: deps.events.clone(),
        })
    }

    /// Every course, most recently opened first, with its progress and the
    /// section it resumes at.
    pub(crate) async fn list_courses(
        &self,
        _cx: &Features,
    ) -> Result<Vec<CourseSummary>, CoreError> {
        self.summaries()
    }

    /// Records that the course was opened now and returns it with its index
    /// and where it resumes, or with where placement stands while it is in
    /// placement.
    pub(crate) async fn open_course(
        &self,
        cx: &Features,
        course_id: Uuid,
    ) -> Result<CourseView, CoreError> {
        self.store.mark_opened(course_id)?;
        let mut course = self.course(course_id)?;
        course.placement = cx.placement.state(course_id)?;
        Ok(course)
    }

    /// Deletes the course and everything under it for good, then tells the
    /// interface the library changed.
    pub(crate) async fn delete_course(
        &self,
        _cx: &Features,
        course_id: Uuid,
    ) -> Result<(), CoreError> {
        self.store.delete_course(course_id)?;
        self.events.emit(CoreEvent::CoursesChanged {
            courses: self.summaries()?,
        });
        Ok(())
    }

    /// A course with its index and where it resumes, without recording that
    /// it was opened.
    pub(crate) fn course(&self, course_id: Uuid) -> Result<CourseView, CoreError> {
        let record = self.record(course_id)?;
        let tree = self.store.index_tree(course_id)?;
        Ok(CourseView {
            id: record.id,
            resume_at: resume_at(&record, &tree).map(|section| section.id),
            topic: record.topic,
            stage: record.stage,
            chapters: chapter_nodes(&tree),
            glossary_count: count(self.store.glossary(course_id)?.len()),
            side_chat_count: count(self.store.side_chats(course_id)?.len()),
            placement: None,
        })
    }

    /// The index of a live course.
    pub(crate) fn tree(&self, course_id: Uuid) -> Result<IndexTree, CoreError> {
        self.record(course_id)?;
        Ok(self.store.index_tree(course_id)?)
    }

    fn record(&self, course_id: Uuid) -> Result<CourseRecord, CoreError> {
        self.store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })
    }

    fn summaries(&self) -> Result<Vec<CourseSummary>, CoreError> {
        self.store
            .course_records()?
            .into_iter()
            .map(|record| {
                let tree = self.store.index_tree(record.id)?;
                Ok(CourseSummary {
                    id: record.id,
                    progress: index_tree::progress(&tree),
                    resume_title: resume_at(&record, &tree).map(|section| section.title.clone()),
                    topic: record.topic,
                    stage: record.stage,
                    last_opened_at: record.last_opened_at,
                    created_at: record.created_at,
                    updated_at: record.updated_at,
                })
            })
            .collect()
    }
}

/// The section a course resumes at: the one last read while it is still in
/// the index, otherwise the first.
fn resume_at<'a>(record: &CourseRecord, tree: &'a IndexTree) -> Option<&'a TreeSection> {
    let sections = index_tree::in_index_order(tree);
    let find = |id: Uuid| {
        sections
            .iter()
            .find(|entry| entry.section.id == id)
            .map(|entry| entry.section)
    };
    record
        .last_read
        .and_then(find)
        .or_else(|| index_tree::first_section(tree).and_then(find))
}

/// The index as the interface draws it: each section with its reading state
/// and its deep dives.
fn chapter_nodes(tree: &IndexTree) -> Vec<ChapterNode> {
    fn node(section: &TreeSection, kind: SectionKind) -> SectionNode {
        SectionNode {
            id: section.id,
            kind,
            title: section.title.clone(),
            summary: section.line.clone(),
            read: section.read,
            mark: section.mark(),
            deep_dives: section
                .children
                .iter()
                .map(|child| node(child, SectionKind::DeepDive))
                .collect(),
        }
    }
    tree.chapters
        .iter()
        .map(|chapter| ChapterNode {
            id: chapter.id,
            title: chapter.title.clone(),
            sections: chapter
                .sections
                .iter()
                .map(|section| node(section, SectionKind::BroadPass))
                .collect(),
        })
        .collect()
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}
