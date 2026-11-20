//! Section versions with their records, the current-version pointer, and
//! check answers.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::index::{ConventionOutcome, conventions, insert_convention};
use super::{
    Id, Store, StoreError, Time, changed_one, id_at, json, json_at, optional_json_at, time_at,
};
use crate::domain::conventions::Convention;
use crate::domain::questions::KeyedCheckQuestion;
use crate::domain::record::{RecordStatus, SectionRecord, StoredRecord};
use crate::protocol::{Block, Citation, Source, VersionStyle};

/// The check questions of one block, with their keys, which the shown block
/// leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckBlock {
    pub block: u32,
    pub questions: Vec<KeyedCheckQuestion>,
}

/// A complete version to store. Its id is the caller's, so the conventions
/// its record fixes can name it as their origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSectionVersion {
    pub id: Uuid,
    /// The section's index node.
    pub node: Uuid,
    pub style: VersionStyle,
    pub blocks: Vec<Block>,
    pub checks: Vec<CheckBlock>,
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
    pub written_without_search: bool,
    /// Confirmed, or predicted from the map entry.
    pub record: StoredRecord,
    /// The model that wrote it.
    pub model: Option<String>,
    /// The prompt and its version.
    pub prompt: String,
}

/// A stored, immutable version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSectionVersion {
    pub id: Uuid,
    pub node_id: Uuid,
    pub style: VersionStyle,
    pub blocks: Vec<Block>,
    pub checks: Vec<CheckBlock>,
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
    pub written_without_search: bool,
    /// Absent only from versions stored before records existed.
    pub record: Option<StoredRecord>,
    pub model: Option<String>,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The first answer given to a check question of a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckAnswerRecord {
    pub id: Uuid,
    pub version_id: Uuid,
    pub block: u32,
    /// The question's index within its block.
    pub question: u32,
    /// The option chosen, counted from 0.
    pub choice: u32,
    pub correct: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Columns of `section_versions` aliased `version`.
const VERSION_COLUMNS: &str = "version.id, version.node_id, version.style, version.blocks, \
     version.checks, version.citations, version.sources, version.written_without_search, \
     version.record, version.record_status, version.model, version.prompt, \
     version.created_at, version.updated_at";

fn version_row(row: &Row<'_>) -> rusqlite::Result<StoredSectionVersion> {
    let record: Option<SectionRecord> = optional_json_at(row, 8)?;
    let status: Option<RecordStatus> = row.get(9)?;
    let record = record
        .zip(status)
        .map(|(record, status)| StoredRecord { record, status });
    Ok(StoredSectionVersion {
        id: id_at(row, 0)?,
        node_id: id_at(row, 1)?,
        style: row.get(2)?,
        blocks: json_at(row, 3)?,
        checks: json_at(row, 4)?,
        citations: json_at(row, 5)?,
        sources: json_at(row, 6)?,
        written_without_search: row.get(7)?,
        record,
        model: row.get(10)?,
        prompt: row.get(11)?,
        created_at: time_at(row, 12)?,
        updated_at: time_at(row, 13)?,
    })
}

const CHECK_COLUMNS: &str =
    "id, version_id, block, question, choice, correct, created_at, updated_at";

fn check_row(row: &Row<'_>) -> rusqlite::Result<CheckAnswerRecord> {
    Ok(CheckAnswerRecord {
        id: id_at(row, 0)?,
        version_id: id_at(row, 1)?,
        block: row.get(2)?,
        question: row.get(3)?,
        choice: row.get(4)?,
        correct: row.get(5)?,
        created_at: time_at(row, 6)?,
        updated_at: time_at(row, 7)?,
    })
}

/// A version to insert; `record` is absent only for the thin loop's versions.
pub(super) struct VersionRow<'a> {
    pub id: Uuid,
    pub node: Uuid,
    pub style: VersionStyle,
    pub blocks: &'a [Block],
    pub checks: &'a [CheckBlock],
    pub citations: &'a [Citation],
    pub sources: &'a [Source],
    pub written_without_search: bool,
    pub record: Option<&'a StoredRecord>,
    pub model: Option<&'a str>,
    pub prompt: &'a str,
}

/// The course of the live section `node`; `NotFound` unless there is one.
fn section_course(connection: &Connection, node: Uuid) -> Result<Uuid, StoreError> {
    let course: Id = connection
        .query_row(
            "SELECT course_id FROM index_nodes
             WHERE id = ?1 AND kind = 'section' AND deleted_at IS NULL",
            [Id(node)],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StoreError::NotFound(format!("section {node}")))?;
    Ok(course.0)
}

/// Inserts a version and makes it its node's current one; fails with
/// `NotFound` unless the node is a live section. Returns the section's course.
pub(super) fn insert_version_row(
    connection: &Connection,
    now: DateTime<Utc>,
    version: &VersionRow<'_>,
) -> Result<Uuid, StoreError> {
    let course = section_course(connection, version.node)?;
    connection.execute(
        "INSERT INTO section_versions
            (id, node_id, style, blocks, checks, citations, sources, written_without_search,
             record, record_status, model, prompt, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            Id(version.id),
            Id(version.node),
            version.style,
            json(version.blocks)?,
            json(version.checks)?,
            json(version.citations)?,
            json(version.sources)?,
            version.written_without_search,
            version
                .record
                .map(|stored| json(&stored.record))
                .transpose()?,
            version.record.map(|stored| stored.status),
            version.model,
            version.prompt,
            Time(now),
        ],
    )?;
    connection.execute(
        "UPDATE index_nodes SET current_version_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![Id(version.node), Id(version.id), Time(now)],
    )?;
    Ok(course)
}

/// Inserts a version with its record and the conventions it newly fixes,
/// and makes it current.
fn insert_with_conventions(
    connection: &Connection,
    now: DateTime<Utc>,
    version: &NewSectionVersion,
    conventions: &[Convention],
) -> Result<Vec<ConventionOutcome>, StoreError> {
    let course = insert_version_row(
        connection,
        now,
        &VersionRow {
            id: version.id,
            node: version.node,
            style: version.style,
            blocks: &version.blocks,
            checks: &version.checks,
            citations: &version.citations,
            sources: &version.sources,
            written_without_search: version.written_without_search,
            record: Some(&version.record),
            model: version.model.as_deref(),
            prompt: &version.prompt,
        },
    )?;
    conventions
        .iter()
        .map(|convention| insert_convention(connection, now, course, convention))
        .collect()
}

impl Store {
    /// Stores a version with its record and the conventions its record newly
    /// fixes, and makes it current, in one transaction. A convention whose
    /// key is already fixed is rejected and the rest is stored.
    pub fn insert_version(
        &self,
        version: &NewSectionVersion,
        conventions: &[Convention],
    ) -> Result<Vec<ConventionOutcome>, StoreError> {
        let now = self.now();
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        let outcomes = insert_with_conventions(&transaction, now, version, conventions)?;
        transaction.commit()?;
        Ok(outcomes)
    }

    /// Stores a version of the section `node` like [`Store::insert_version`],
    /// built from the conventions of its course as they stand inside the same
    /// transaction: `build` returns the version and the conventions it newly
    /// fixes, so a convention fixed by a version stored meanwhile is never
    /// missed.
    pub fn insert_version_with(
        &self,
        node: Uuid,
        build: impl FnOnce(&[Convention]) -> (NewSectionVersion, Vec<Convention>),
    ) -> Result<Vec<ConventionOutcome>, StoreError> {
        let now = self.now();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let fixed = conventions(&transaction, section_course(&transaction, node)?)?;
        let (version, new) = build(&fixed);
        if version.node != node {
            return Err(StoreError::NotFound(format!(
                "version {} of section {node}",
                version.id
            )));
        }
        let outcomes = insert_with_conventions(&transaction, now, &version, &new)?;
        transaction.commit()?;
        Ok(outcomes)
    }

    /// Makes one of a section's own versions its current one.
    pub fn set_current_version(&self, node: Uuid, version: Uuid) -> Result<(), StoreError> {
        let changed = self.connection().execute(
            "UPDATE index_nodes SET current_version_id = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL
               AND EXISTS (SELECT 1 FROM section_versions
                           WHERE id = ?2 AND node_id = ?1 AND deleted_at IS NULL)",
            params![Id(node), Id(version), Time(self.now())],
        )?;
        changed_one(changed, || format!("version {version} of section {node}"))
    }

    pub fn version(&self, version: Uuid) -> Result<Option<StoredSectionVersion>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                &format!(
                    "SELECT {VERSION_COLUMNS} FROM section_versions version
                     WHERE version.id = ?1 AND version.deleted_at IS NULL"
                ),
                [Id(version)],
                version_row,
            )
            .optional()?)
    }

    /// A section's versions, oldest first.
    pub fn versions(&self, node: Uuid) -> Result<Vec<StoredSectionVersion>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {VERSION_COLUMNS} FROM section_versions version
             WHERE version.node_id = ?1 AND version.deleted_at IS NULL
             ORDER BY version.created_at, version.rowid"
        ))?;
        let versions = statement
            .query_map([Id(node)], version_row)?
            .collect::<Result<_, _>>()?;
        Ok(versions)
    }

    /// The version the section's pointer names.
    pub fn current_version(&self, node: Uuid) -> Result<Option<StoredSectionVersion>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                &format!(
                    "SELECT {VERSION_COLUMNS} FROM index_nodes node
                     JOIN section_versions version
                       ON version.id = node.current_version_id AND version.deleted_at IS NULL
                     WHERE node.id = ?1 AND node.deleted_at IS NULL"
                ),
                [Id(node)],
                version_row,
            )
            .optional()?)
    }

    /// The record of the section's current version.
    pub fn current_record(&self, node: Uuid) -> Result<Option<StoredRecord>, StoreError> {
        Ok(self
            .current_version(node)?
            .and_then(|version| version.record))
    }

    /// Stores the answer to a check question.
    pub fn add_check_answer(
        &self,
        version: Uuid,
        block: u32,
        question: u32,
        choice: u32,
        correct: bool,
    ) -> Result<CheckAnswerRecord, StoreError> {
        let now = self.now();
        let id = Uuid::new_v4();
        self.connection().execute(
            "INSERT INTO check_answers
                (id, version_id, block, question, choice, correct, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                Id(id),
                Id(version),
                block,
                question,
                choice,
                correct,
                Time(now)
            ],
        )?;
        Ok(CheckAnswerRecord {
            id,
            version_id: version,
            block,
            question,
            choice,
            correct,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn check_answer(
        &self,
        version: Uuid,
        block: u32,
        question: u32,
    ) -> Result<Option<CheckAnswerRecord>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                &format!(
                    "SELECT {CHECK_COLUMNS} FROM check_answers
                     WHERE version_id = ?1 AND block = ?2 AND question = ?3
                       AND deleted_at IS NULL"
                ),
                params![Id(version), block, question],
                check_row,
            )
            .optional()?)
    }

    /// The answers given on a version, by block and question.
    pub fn check_answers(&self, version: Uuid) -> Result<Vec<CheckAnswerRecord>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {CHECK_COLUMNS} FROM check_answers
             WHERE version_id = ?1 AND deleted_at IS NULL ORDER BY block, question"
        ))?;
        let answers = statement
            .query_map([Id(version)], check_row)?
            .collect::<Result<_, _>>()?;
        Ok(answers)
    }
}
