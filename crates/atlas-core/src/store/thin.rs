//! The views the thin loop's sections feature still reads and writes: a
//! section with its current version as one text. The sections feature moves
//! to the repository functions of the other files.

use rusqlite::OptionalExtension;
use uuid::Uuid;

use super::sections::{VersionRow, insert_version_row};
use super::{Id, Store, StoreError, json_at, time_at};
use crate::protocol::{Block, BlockContent, VersionStyle};

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
