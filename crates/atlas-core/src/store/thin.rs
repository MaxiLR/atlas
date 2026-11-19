//! The views the thin loop's features read and write: the library, a
//! course's index and a section with its current version as one text. The
//! feature tickets move to the repository functions of the other files.

use rusqlite::OptionalExtension;
use uuid::Uuid;

use super::sections::{VersionRow, insert_version_row};
use super::{Id, Store, StoreError, id_at, json_at, optional_time_at, time_at};
use crate::protocol::{
    Block, BlockContent, ChapterNode, CourseStage, CourseSummary, CourseView, NodeMark, Progress,
    SectionKind, SectionNode, VersionStyle,
};

/// A section of the index, with its current version if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub summary: String,
    pub latest: Option<StoredVersion>,
}

/// A version's prose as one text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredVersion {
    pub id: Uuid,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Store {
    /// The library, most recently updated first.
    pub fn list_courses(&self) -> Result<Vec<CourseSummary>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT course.id, course.topic, course.stage, course.last_opened_at,
                    course.created_at, course.updated_at,
                    (SELECT COUNT(*) FROM index_nodes node
                     WHERE node.course_id = course.id AND node.kind = 'section'
                       AND node.deleted_at IS NULL),
                    (SELECT COUNT(*) FROM index_nodes node
                     WHERE node.course_id = course.id AND node.kind = 'section'
                       AND node.deleted_at IS NULL
                       AND EXISTS (SELECT 1 FROM section_versions version
                                   WHERE version.node_id = node.id
                                     AND version.deleted_at IS NULL))
             FROM courses course
             WHERE course.deleted_at IS NULL
             ORDER BY course.updated_at DESC, course.created_at DESC",
        )?;
        let courses = statement
            .query_map([], |row| {
                Ok(CourseSummary {
                    id: id_at(row, 0)?,
                    topic: row.get(1)?,
                    stage: row.get(2)?,
                    last_opened_at: optional_time_at(row, 3)?,
                    created_at: time_at(row, 4)?,
                    updated_at: time_at(row, 5)?,
                    progress: Progress {
                        total: row.get(6)?,
                        read: row.get(7)?,
                    },
                    resume_title: None,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(courses)
    }

    /// A course with its index, or `None` if there is no such course. A
    /// section counts as read once a version of it is stored.
    pub fn course(&self, course_id: Uuid) -> Result<Option<CourseView>, StoreError> {
        let connection = self.connection();
        let course: Option<(String, CourseStage)> = connection
            .query_row(
                "SELECT topic, stage FROM courses WHERE id = ?1 AND deleted_at IS NULL",
                [Id(course_id)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((topic, stage)) = course else {
            return Ok(None);
        };
        let mut statement = connection.prepare(
            "SELECT chapter.id, chapter.title,
                    section.id, section.title, section.summary,
                    EXISTS (SELECT 1 FROM section_versions version
                            WHERE version.node_id = section.id AND version.deleted_at IS NULL)
             FROM index_nodes chapter
             JOIN index_nodes section
               ON section.parent_id = chapter.id AND section.deleted_at IS NULL
             WHERE chapter.course_id = ?1 AND chapter.kind = 'chapter'
               AND chapter.deleted_at IS NULL
             ORDER BY chapter.position, section.position",
        )?;
        let mut rows = statement.query([Id(course_id)])?;
        let mut chapters: Vec<ChapterNode> = Vec::new();
        while let Some(row) = rows.next()? {
            let chapter_id = id_at(row, 0)?;
            if chapters
                .last()
                .is_none_or(|chapter| chapter.id != chapter_id)
            {
                chapters.push(ChapterNode {
                    id: chapter_id,
                    title: row.get(1)?,
                    sections: Vec::new(),
                });
            }
            let section = section_node(id_at(row, 2)?, row.get(3)?, row.get(4)?, row.get(5)?);
            chapters
                .last_mut()
                .expect("a chapter was just pushed")
                .sections
                .push(section);
        }
        Ok(Some(course_view(course_id, topic, stage, chapters)))
    }

    /// A section of a course with its current version's prose.
    pub fn section(
        &self,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<Option<StoredSection>, StoreError> {
        let row = self
            .connection()
            .query_row(
                "SELECT node.title, node.summary, version.id, version.blocks, version.created_at
                 FROM index_nodes node
                 LEFT JOIN section_versions version
                   ON version.id = node.current_version_id AND version.deleted_at IS NULL
                 WHERE node.id = ?1 AND node.course_id = ?2 AND node.kind = 'section'
                   AND node.deleted_at IS NULL",
                [Id(section_id), Id(course_id)],
                |row| {
                    let latest = match row.get::<_, Option<Id>>(2)? {
                        None => None,
                        Some(Id(id)) => Some(StoredVersion {
                            id,
                            content: prose(&json_at::<Vec<Block>>(row, 3)?),
                            created_at: time_at(row, 4)?,
                        }),
                    };
                    Ok(StoredSection {
                        id: section_id,
                        course_id,
                        title: row.get(0)?,
                        summary: row.get(1)?,
                        latest,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Stores a version of a section as one prose block, written with
    /// `prompt`, and makes it current.
    pub fn insert_section_version(
        &self,
        section_id: Uuid,
        content: &str,
        prompt: &str,
    ) -> Result<(), StoreError> {
        let blocks = [Block {
            index: 0,
            content: BlockContent::Prose {
                text: content.to_owned(),
            },
        }];
        let now = self.now();
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        insert_version_row(
            &transaction,
            now,
            &VersionRow {
                id: Uuid::new_v4(),
                node: section_id,
                style: VersionStyle::Original,
                blocks: &blocks,
                checks: &[],
                citations: &[],
                sources: &[],
                written_without_search: false,
                record: None,
                model: None,
                prompt,
            },
        )?;
        transaction.commit()?;
        Ok(())
    }
}

/// The text of a version's prose blocks.
fn prose(blocks: &[Block]) -> String {
    blocks
        .iter()
        .filter_map(|block| match &block.content {
            BlockContent::Prose { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn section_node(id: Uuid, title: String, summary: String, read: bool) -> SectionNode {
    SectionNode {
        id,
        kind: SectionKind::BroadPass,
        title,
        summary,
        read,
        mark: if read {
            NodeMark::Read
        } else {
            NodeMark::Unread
        },
        deep_dives: Vec::new(),
    }
}

fn course_view(
    id: Uuid,
    topic: String,
    stage: CourseStage,
    chapters: Vec<ChapterNode>,
) -> CourseView {
    CourseView {
        id,
        topic,
        stage,
        chapters,
        resume_at: None,
        glossary_count: 0,
        side_chat_count: 0,
        placement: None,
    }
}
