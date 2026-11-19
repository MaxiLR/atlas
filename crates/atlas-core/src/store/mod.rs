//! SQLite store with versioned, forward-only migrations. The database path is
//! injected; nothing here knows where the app keeps its data.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use crate::harness::index::IndexDraft;
use crate::protocol::{ChapterView, CourseSummary, CourseView, SectionEntry};

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

/// A section of the index, with its stored content if it has been generated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub summary: String,
    pub content: Option<String>,
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
                sections.push(SectionEntry {
                    id: section_id,
                    title: section.title.clone(),
                    summary: section.summary.clone(),
                    generated: false,
                });
            }
            chapters.push(ChapterView {
                id: chapter_id,
                title: chapter.title.clone(),
                sections,
            });
        }
        drop(node);
        transaction.commit()?;
        Ok(CourseView {
            id: course_id,
            topic: topic.to_owned(),
            chapters,
        })
    }

    /// The library, most recently updated first.
    pub fn list_courses(&self) -> Result<Vec<CourseSummary>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, topic, created_at, updated_at FROM courses
             WHERE deleted_at IS NULL
             ORDER BY updated_at DESC, created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, topic, created_at, updated_at) = row?;
            Ok(CourseSummary {
                id: parse_id(&id)?,
                topic,
                created_at: parse_time(&created_at)?,
                updated_at: parse_time(&updated_at)?,
            })
        })
        .collect()
    }

    /// A course with its index, or `None` if there is no such course.
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
        let mut chapters: Vec<ChapterView> = Vec::new();
        while let Some(row) = rows.next()? {
            let chapter_id = parse_id(&row.get::<_, String>(0)?)?;
            if chapters
                .last()
                .is_none_or(|chapter| chapter.id != chapter_id)
            {
                chapters.push(ChapterView {
                    id: chapter_id,
                    title: row.get(1)?,
                    sections: Vec::new(),
                });
            }
            let section = SectionEntry {
                id: parse_id(&row.get::<_, String>(2)?)?,
                title: row.get(3)?,
                summary: row.get(4)?,
                generated: row.get(5)?,
            };
            chapters
                .last_mut()
                .expect("a chapter was just pushed")
                .sections
                .push(section);
        }
        Ok(Some(CourseView {
            id: course_id,
            topic,
            chapters,
        }))
    }

    /// A section of a course with its latest stored content.
    pub fn section(
        &self,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<Option<StoredSection>, StoreError> {
        self.connection()
            .query_row(
                "SELECT title, summary,
                        (SELECT content FROM section_versions version
                         WHERE version.node_id = node.id AND version.deleted_at IS NULL
                         ORDER BY version.created_at DESC LIMIT 1)
                 FROM index_nodes node
                 WHERE node.id = ?1 AND node.course_id = ?2 AND node.kind = 'section'
                   AND node.deleted_at IS NULL",
                [section_id.to_string(), course_id.to_string()],
                |row: &Row<'_>| {
                    Ok(StoredSection {
                        id: section_id,
                        course_id,
                        title: row.get(0)?,
                        summary: row.get(1)?,
                        content: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
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
