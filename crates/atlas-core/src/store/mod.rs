//! SQLite store with versioned, forward-only migrations. The database path is
//! injected; nothing here knows where the app keeps its data.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::harness::index::IndexDraft;
use crate::protocol::{
    ChapterNode, CoreError, CourseStage, CourseSummary, CourseView, NodeMark, Progress,
    SectionKind, SectionNode,
};

/// Migration `n` (1-based) is `MIGRATIONS[n - 1]`; `PRAGMA user_version`
/// records the last one applied.
const MIGRATIONS: [&str; 1] = [include_str!("migrations/0001_thin_loop.sql")];
/// The schema version this code migrates to.
const LATEST: i64 = MIGRATIONS.len() as i64;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("the database is at version {found}, newer than this app's {supported}")]
    TooNew { found: i64, supported: i64 },
    #[error("corrupt record: {0}")]
    Corrupt(String),
}

impl From<StoreError> for CoreError {
    fn from(error: StoreError) -> Self {
        CoreError::Store {
            message: error.to_string(),
        }
    }
}

/// A section of the index, with its latest stored version if it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub summary: String,
    pub latest: Option<StoredVersion>,
}

/// A complete version of a section as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredVersion {
    pub id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// Opens (creating if missing) the database at `path` and migrates it.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection.lock().expect("store lock poisoned")
    }

    /// Stores a new course with its index, in one transaction.
    pub fn insert_course(&self, topic: &str, index: &IndexDraft) -> Result<CourseView, StoreError> {
        let now = timestamp(Utc::now());
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        let course_id = Uuid::new_v4();
        transaction.execute(
            "INSERT INTO courses (id, topic, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![course_id.to_string(), topic, now],
        )?;
        let mut node = transaction.prepare(
            "INSERT INTO index_nodes
                (id, course_id, parent_id, kind, position, title, summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        )?;
        let mut chapters = Vec::with_capacity(index.chapters.len());
        for (chapter_position, chapter) in (0_i64..).zip(&index.chapters) {
            let chapter_id = Uuid::new_v4();
            node.execute(params![
                chapter_id.to_string(),
                course_id.to_string(),
                None::<String>,
                "chapter",
                chapter_position,
                chapter.title,
                "",
                now,
            ])?;
            let mut sections = Vec::with_capacity(chapter.sections.len());
            for (section_position, section) in (0_i64..).zip(&chapter.sections) {
                let section_id = Uuid::new_v4();
                node.execute(params![
                    section_id.to_string(),
                    course_id.to_string(),
                    chapter_id.to_string(),
                    "section",
                    section_position,
                    section.title,
                    section.summary,
                    now,
                ])?;
                sections.push(section_node(
                    section_id,
                    section.title.clone(),
                    section.summary.clone(),
                    false,
                ));
            }
            chapters.push(ChapterNode {
                id: chapter_id,
                title: chapter.title.clone(),
                sections,
            });
        }
        drop(node);
        transaction.commit()?;
        Ok(course_view(course_id, topic.to_owned(), chapters))
    }

    /// The library, most recently updated first.
    pub fn list_courses(&self) -> Result<Vec<CourseSummary>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT course.id, course.topic, course.created_at, course.updated_at,
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
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
            ))
        })?;
        rows.map(|row| {
            let (id, topic, created_at, updated_at, total, read) = row?;
            Ok(CourseSummary {
                id: parse_id(&id)?,
                topic,
                stage: CourseStage::Ready,
                progress: Progress { read, total },
                resume_title: None,
                last_opened_at: None,
                created_at: parse_time(&created_at)?,
                updated_at: parse_time(&updated_at)?,
            })
        })
        .collect()
    }

    /// A course with its index, or `None` if there is no such course. A
    /// section counts as read once a version of it is stored.
    pub fn course(&self, course_id: Uuid) -> Result<Option<CourseView>, StoreError> {
        let connection = self.connection();
        let topic: Option<String> = connection
            .query_row(
                "SELECT topic FROM courses WHERE id = ?1 AND deleted_at IS NULL",
                [course_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(topic) = topic else {
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
        let mut rows = statement.query([course_id.to_string()])?;
        let mut chapters: Vec<ChapterNode> = Vec::new();
        while let Some(row) = rows.next()? {
            let chapter_id = parse_id(&row.get::<_, String>(0)?)?;
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
            let section = section_node(
                parse_id(&row.get::<_, String>(2)?)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            );
            chapters
                .last_mut()
                .expect("a chapter was just pushed")
                .sections
                .push(section);
        }
        Ok(Some(course_view(course_id, topic, chapters)))
    }

    /// A section of a course with its latest stored version.
    pub fn section(
        &self,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<Option<StoredSection>, StoreError> {
        let row = self
            .connection()
            .query_row(
                "SELECT node.title, node.summary, version.id, version.content, version.created_at
                 FROM index_nodes node
                 LEFT JOIN section_versions version
                   ON version.id = (SELECT latest.id FROM section_versions latest
                                    WHERE latest.node_id = node.id AND latest.deleted_at IS NULL
                                    ORDER BY latest.created_at DESC LIMIT 1)
                 WHERE node.id = ?1 AND node.course_id = ?2 AND node.kind = 'section'
                   AND node.deleted_at IS NULL",
                [section_id.to_string(), course_id.to_string()],
                |row: &Row<'_>| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((title, summary, version_id, content, created_at)) = row else {
            return Ok(None);
        };
        let latest = match (version_id, content, created_at) {
            (Some(id), Some(content), Some(created_at)) => Some(StoredVersion {
                id: parse_id(&id)?,
                content,
                created_at: parse_time(&created_at)?,
            }),
            _ => None,
        };
        Ok(Some(StoredSection {
            id: section_id,
            course_id,
            title,
            summary,
            latest,
        }))
    }

    /// Stores a complete version of a section, written with `prompt`.
    pub fn insert_section_version(
        &self,
        section_id: Uuid,
        content: &str,
        prompt: &str,
    ) -> Result<(), StoreError> {
        let now = timestamp(Utc::now());
        self.connection().execute(
            "INSERT INTO section_versions (id, node_id, content, prompt, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                Uuid::new_v4().to_string(),
                section_id.to_string(),
                content,
                prompt,
                now
            ],
        )?;
        Ok(())
    }
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

fn course_view(id: Uuid, topic: String, chapters: Vec<ChapterNode>) -> CourseView {
    CourseView {
        id,
        topic,
        stage: CourseStage::Ready,
        chapters,
        resume_at: None,
        glossary_count: 0,
        side_chat_count: 0,
        placement: None,
    }
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let applied: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if applied > LATEST {
        return Err(StoreError::TooNew {
            found: applied,
            supported: LATEST,
        });
    }
    for (version, sql) in (1_i64..)
        .zip(MIGRATIONS)
        .skip_while(|(version, _)| *version <= applied)
    {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", version)?;
        transaction.commit()?;
    }
    Ok(())
}

fn timestamp(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn parse_time(text: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(text)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| StoreError::Corrupt(format!("timestamp {text:?}: {error}")))
}

fn parse_id(text: &str) -> Result<Uuid, StoreError> {
    Uuid::parse_str(text).map_err(|error| StoreError::Corrupt(format!("id {text:?}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_database_migrates_to_the_latest_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        Store::open(&path).unwrap();
        let version: i64 = Connection::open(&path)
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, LATEST);
        // Opening again applies nothing twice.
        Store::open(&path).unwrap();
    }

    #[test]
    fn a_database_newer_than_the_code_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", LATEST + 1)
            .unwrap();
        assert!(matches!(
            Store::open(&path),
            Err(StoreError::TooNew { found, supported })
                if found == LATEST + 1 && supported == LATEST
        ));
    }
}
