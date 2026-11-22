//! The course glossary: definitions anchored to a passage of one version,
//! with the Wikipedia lookup once made.

use rusqlite::{OptionalExtension, Row, params};
use uuid::Uuid;

use super::{Id, Store, StoreError, Time, changed_one, id_at, json, optional_json_at, time_at};
use crate::protocol::{Anchor, GlossaryEntry, WikipediaResult};

/// A definition to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGlossaryEntry {
    pub course: Uuid,
    pub term: String,
    pub definition: String,
    pub anchor: Anchor,
}

/// Live entries with the section their version belongs to; the caller adds
/// its conditions with `AND`.
const ENTRY_SELECT: &str = "SELECT entry.id, entry.course_id, node.id, node.title, entry.term,
            entry.definition, entry.version_id, entry.block, entry.quote, entry.wikipedia,
            entry.created_at
     FROM glossary_entries entry
     JOIN section_versions version ON version.id = entry.version_id
     JOIN index_nodes node ON node.id = version.node_id
     WHERE entry.deleted_at IS NULL";

fn entry_row(row: &Row<'_>) -> rusqlite::Result<GlossaryEntry> {
    Ok(GlossaryEntry {
        id: id_at(row, 0)?,
        course_id: id_at(row, 1)?,
        section_id: id_at(row, 2)?,
        section_title: row.get(3)?,
        term: row.get(4)?,
        definition: row.get(5)?,
        anchor: Anchor {
            version_id: id_at(row, 6)?,
            block: row.get(7)?,
            quote: row.get(8)?,
        },
        wikipedia: optional_json_at(row, 9)?,
        created_at: time_at(row, 10)?,
    })
}

impl Store {
    /// Stores a definition anchored to a version of a section of the course.
    pub fn add_glossary_entry(
        &self,
        entry: &NewGlossaryEntry,
    ) -> Result<GlossaryEntry, StoreError> {
        let now = self.now();
        let connection = self.connection();
        let anchor = &entry.anchor;
        let (section_id, section_title): (Id, String) = connection
            .query_row(
                "SELECT node.id, node.title FROM section_versions version
                 JOIN index_nodes node ON node.id = version.node_id
                 WHERE version.id = ?1 AND node.course_id = ?2 AND version.deleted_at IS NULL",
                params![Id(anchor.version_id), Id(entry.course)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                StoreError::NotFound(format!(
                    "version {} in course {}",
                    anchor.version_id, entry.course
                ))
            })?;
        let id = Uuid::new_v4();
        connection.execute(
            "INSERT INTO glossary_entries
                (id, course_id, version_id, block, quote, term, definition,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                Id(id),
                Id(entry.course),
                Id(anchor.version_id),
                anchor.block,
                anchor.quote,
                entry.term,
                entry.definition,
                Time(now)
            ],
        )?;
        Ok(GlossaryEntry {
            id,
            course_id: entry.course,
            section_id: section_id.0,
            section_title,
            term: entry.term.clone(),
            definition: entry.definition.clone(),
            anchor: anchor.clone(),
            wikipedia: None,
            created_at: now,
        })
    }

    /// The course's glossary, in the order defined.
    pub fn glossary(&self, course: Uuid) -> Result<Vec<GlossaryEntry>, StoreError> {
        self.glossary_entries("AND entry.course_id = ?1", params![Id(course)])
    }

    pub fn glossary_entry(&self, entry: Uuid) -> Result<Option<GlossaryEntry>, StoreError> {
        Ok(self
            .glossary_entries("AND entry.id = ?1", params![Id(entry)])?
            .pop())
    }

    /// The entry defined at exactly this anchor.
    pub fn glossary_entry_for_anchor(
        &self,
        anchor: &Anchor,
    ) -> Result<Option<GlossaryEntry>, StoreError> {
        Ok(self
            .glossary_entries(
                "AND entry.version_id = ?1 AND entry.block = ?2 AND entry.quote = ?3",
                params![Id(anchor.version_id), anchor.block, anchor.quote],
            )?
            .into_iter()
            .next())
    }

    /// The entries anchored to one version, which show only on it.
    pub fn glossary_for_version(&self, version: Uuid) -> Result<Vec<GlossaryEntry>, StoreError> {
        self.glossary_entries("AND entry.version_id = ?1", params![Id(version)])
    }

    /// Stores the Wikipedia lookup made for an entry.
    pub fn set_wikipedia(&self, entry: Uuid, result: &WikipediaResult) -> Result<(), StoreError> {
        let changed = self.connection().execute(
            "UPDATE glossary_entries SET wikipedia = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![Id(entry), json(result)?, Time(self.now())],
        )?;
        changed_one(changed, || format!("glossary entry {entry}"))
    }

    /// Deletes a glossary entry for good; not built yet.
    pub fn delete_glossary_entry(&self, entry: Uuid) -> Result<(), StoreError> {
        Err(StoreError::NotFound(format!("glossary entry {entry}")))
    }

    fn glossary_entries(
        &self,
        condition: &str,
        parameters: &[&dyn rusqlite::ToSql],
    ) -> Result<Vec<GlossaryEntry>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "{ENTRY_SELECT} {condition} ORDER BY entry.created_at, entry.rowid"
        ))?;
        let entries = statement
            .query_map(parameters, entry_row)?
            .collect::<Result<_, _>>()?;
        Ok(entries)
    }
}
