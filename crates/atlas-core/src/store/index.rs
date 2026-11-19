//! The index tree, the course map and the conventions: the plan stored with
//! the course.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use super::courses::require_course;
use super::{
    Id, Store, StoreError, Time, changed_one, id_at, optional_id_at, optional_time_at, time_at,
};
use crate::domain::conventions::{Convention, ConventionOrigin};
use crate::domain::course_map::{Concept, CourseMap, MapEntry};
use crate::domain::ladder::RungId;
use crate::index_tree::{IndexTree, TreeChapter, TreeSection};
use crate::protocol::CourseStage;

/// A chapter of the broad pass, or a section at any depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Chapter,
    Section,
}

sql_text!(NodeKind {
    NodeKind::Chapter => "chapter",
    NodeKind::Section => "section",
});

/// One node of the index as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
    pub id: Uuid,
    pub course_id: Uuid,
    /// The chapter or section it sits under; `None` for a chapter.
    pub parent: Option<Uuid>,
    pub kind: NodeKind,
    /// Among its siblings, from 0.
    pub position: u32,
    pub title: String,
    /// One line on what it covers; empty for a chapter.
    pub line: String,
    /// When a complete version of it was first read.
    pub read_at: Option<DateTime<Utc>>,
    pub current_version: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Whether a convention was stored, or rejected because its key, normalised,
/// is already fixed for the course and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConventionOutcome {
    Stored,
    Rejected,
}

const NODE_COLUMNS: &str = "id, course_id, parent_id, kind, position, title, summary, read_at, \
                            current_version_id, created_at, updated_at";

fn node_row(row: &Row<'_>) -> rusqlite::Result<NodeRecord> {
    Ok(NodeRecord {
        id: id_at(row, 0)?,
        course_id: id_at(row, 1)?,
        parent: optional_id_at(row, 2)?,
        kind: row.get(3)?,
        position: row.get(4)?,
        title: row.get(5)?,
        line: row.get(6)?,
        read_at: optional_time_at(row, 7)?,
        current_version: optional_id_at(row, 8)?,
        created_at: time_at(row, 9)?,
        updated_at: time_at(row, 10)?,
    })
}

/// A node to insert.
pub(super) struct NewNode<'a> {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub kind: NodeKind,
    pub position: u32,
    pub title: &'a str,
    pub line: &'a str,
    pub read: bool,
}

pub(super) fn insert_node(
    connection: &Connection,
    now: DateTime<Utc>,
    course: Uuid,
    node: &NewNode<'_>,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO index_nodes
            (id, course_id, parent_id, kind, position, title, summary, read_at,
             created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            Id(node.id),
            Id(course),
            node.parent.map(Id),
            node.kind,
            node.position,
            node.title,
            node.line,
            node.read.then_some(Time(now)),
            Time(now),
        ],
    )?;
    Ok(())
}

/// Inserts `section` at `position` under `parent`, then its children.
fn insert_section(
    connection: &Connection,
    now: DateTime<Utc>,
    course: Uuid,
    parent: Uuid,
    position: u32,
    section: &TreeSection,
) -> Result<(), StoreError> {
    insert_node(
        connection,
        now,
        course,
        &NewNode {
            id: section.id,
            parent: Some(parent),
            kind: NodeKind::Section,
            position,
            title: &section.title,
            line: &section.line,
            read: section.read,
        },
    )?;
    for (position, child) in (0..).zip(&section.children) {
        insert_section(connection, now, course, section.id, position, child)?;
    }
    Ok(())
}

fn insert_concepts(
    connection: &Connection,
    now: DateTime<Utc>,
    course: Uuid,
    concepts: &[Concept],
) -> Result<(), StoreError> {
    let mut insert = connection.prepare(
        "INSERT INTO concepts (id, course_id, name, rung, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
    )?;
    for concept in concepts {
        insert.execute(params![
            Id(Uuid::new_v4()),
            Id(course),
            concept.name,
            concept.rung.map(|rung| rung.0),
            Time(now)
        ])?;
    }
    Ok(())
}

/// Inserts map entries; every concept they name must be stored already.
fn insert_entries(
    connection: &Connection,
    now: DateTime<Utc>,
    course: Uuid,
    entries: &[MapEntry],
) -> Result<(), StoreError> {
    let mut entry_insert = connection.prepare(
        "INSERT INTO map_entries (id, course_id, node_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
    )?;
    let mut concept_id = connection.prepare(
        "SELECT id FROM concepts WHERE course_id = ?1 AND name = ?2 AND deleted_at IS NULL",
    )?;
    let mut link_insert = connection.prepare(
        "INSERT INTO map_entry_concepts
            (id, entry_id, concept_id, role, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
    )?;
    for entry in entries {
        let entry_id = Uuid::new_v4();
        entry_insert.execute(params![
            Id(entry_id),
            Id(course),
            Id(entry.section),
            Time(now)
        ])?;
        for (role, names) in [
            (INTRODUCES, &entry.introduces),
            (RELIES_ON, &entry.relies_on),
        ] {
            for (position, name) in (0_u32..).zip(names) {
                let concept: Id = concept_id
                    .query_row(params![Id(course), name], |row| row.get(0))
                    .optional()?
                    .ok_or_else(|| StoreError::UnknownConcept(name.clone()))?;
                link_insert.execute(params![
                    Id(Uuid::new_v4()),
                    Id(entry_id),
                    concept,
                    role,
                    position,
                    Time(now)
                ])?;
            }
        }
    }
    Ok(())
}

const INTRODUCES: &str = "introduces";
const RELIES_ON: &str = "relies_on";

/// Stores a convention unless its normalised key is already fixed for the
/// course and kind; the database normalises the key and its unique index
/// decides.
pub(super) fn insert_convention(
    connection: &Connection,
    now: DateTime<Utc>,
    course: Uuid,
    convention: &Convention,
) -> Result<ConventionOutcome, StoreError> {
    let origin = match convention.origin {
        ConventionOrigin::Plan => None,
        ConventionOrigin::SectionVersion { version } => Some(Id(version)),
    };
    let changed = connection.execute(
        "INSERT INTO conventions
            (id, course_id, kind, key, meaning, origin_version_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT DO NOTHING",
        params![
            Id(Uuid::new_v4()),
            Id(course),
            convention.kind,
            convention.key,
            convention.meaning,
            origin,
            Time(now)
        ],
    )?;
    Ok(if changed == 0 {
        ConventionOutcome::Rejected
    } else {
        ConventionOutcome::Stored
    })
}

/// Map entries of live nodes, in the order stored; `node` narrows them to one.
fn map_entries(
    connection: &Connection,
    course: Uuid,
    node: Option<Uuid>,
) -> Result<Vec<MapEntry>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT entry.id, entry.node_id, link.role, concept.name
         FROM map_entries entry
         JOIN index_nodes node ON node.id = entry.node_id AND node.deleted_at IS NULL
         LEFT JOIN map_entry_concepts link
           ON link.entry_id = entry.id AND link.deleted_at IS NULL
         LEFT JOIN concepts concept
           ON concept.id = link.concept_id AND concept.deleted_at IS NULL
         WHERE entry.course_id = ?1 AND entry.deleted_at IS NULL
           AND (?2 IS NULL OR entry.node_id = ?2)
         ORDER BY entry.rowid, link.position",
    )?;
    let mut rows = statement.query(params![Id(course), node.map(Id)])?;
    let mut entries: Vec<(Uuid, MapEntry)> = Vec::new();
    while let Some(row) = rows.next()? {
        let entry_id = id_at(row, 0)?;
        if entries.last().is_none_or(|(id, _)| *id != entry_id) {
            entries.push((
                entry_id,
                MapEntry {
                    section: id_at(row, 1)?,
                    introduces: Vec::new(),
                    relies_on: Vec::new(),
                },
            ));
        }
        let role: Option<String> = row.get(2)?;
        let name: Option<String> = row.get(3)?;
        let (_, entry) = entries.last_mut().expect("an entry was just pushed");
        match (role.as_deref(), name) {
            (Some(INTRODUCES), Some(name)) => entry.introduces.push(name),
            (Some(RELIES_ON), Some(name)) => entry.relies_on.push(name),
            _ => {}
        }
    }
    Ok(entries.into_iter().map(|(_, entry)| entry).collect())
}

impl Store {
    /// Stores the index of a course with its map and the conventions the
    /// plan fixed, in one transaction, and moves the course to the ready
    /// stage. Node ids are the tree's; a section marked read is stored as
    /// read now.
    pub fn insert_index(
        &self,
        course: Uuid,
        tree: &IndexTree,
        map: &CourseMap,
        conventions: &[Convention],
    ) -> Result<Vec<ConventionOutcome>, StoreError> {
        let now = self.now();
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        require_course(&transaction, course)?;
        for (position, chapter) in (0..).zip(&tree.chapters) {
            insert_node(
                &transaction,
                now,
                course,
                &NewNode {
                    id: chapter.id,
                    parent: None,
                    kind: NodeKind::Chapter,
                    position,
                    title: &chapter.title,
                    line: "",
                    read: false,
                },
            )?;
            for (position, section) in (0..).zip(&chapter.sections) {
                insert_section(&transaction, now, course, chapter.id, position, section)?;
            }
        }
        insert_concepts(&transaction, now, course, &map.concepts)?;
        insert_entries(&transaction, now, course, &map.entries)?;
        let outcomes = conventions
            .iter()
            .map(|convention| insert_convention(&transaction, now, course, convention))
            .collect::<Result<_, _>>()?;
        transaction.execute(
            "UPDATE courses SET stage = ?2, updated_at = ?3 WHERE id = ?1 AND stage <> ?2",
            params![Id(course), CourseStage::Ready, Time(now)],
        )?;
        transaction.commit()?;
        Ok(outcomes)
    }

    /// Stores child sections after the existing children of `parent`, with
    /// their new concepts and their map entries, in one transaction. Nothing
    /// stored before is changed.
    pub fn insert_children(
        &self,
        course: Uuid,
        parent: Uuid,
        children: &[TreeSection],
        concepts: &[Concept],
        entries: &[MapEntry],
    ) -> Result<(), StoreError> {
        let now = self.now();
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        let first: u32 = transaction
            .query_row(
                "SELECT (SELECT COALESCE(MAX(child.position) + 1, 0) FROM index_nodes child
                         WHERE child.parent_id = node.id AND child.deleted_at IS NULL)
                 FROM index_nodes node
                 WHERE node.id = ?1 AND node.course_id = ?2 AND node.kind = 'section'
                   AND node.deleted_at IS NULL",
                params![Id(parent), Id(course)],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("section {parent}")))?;
        for (position, child) in (first..).zip(children) {
            insert_section(&transaction, now, course, parent, position, child)?;
        }
        insert_concepts(&transaction, now, course, concepts)?;
        insert_entries(&transaction, now, course, entries)?;
        transaction.commit()?;
        Ok(())
    }

    /// The course's index as a tree: chapters, sections and deep dives in
    /// order. Empty while the course has no index.
    pub fn index_tree(&self, course: Uuid) -> Result<IndexTree, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {NODE_COLUMNS} FROM index_nodes
             WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY position, rowid"
        ))?;
        let nodes: Vec<NodeRecord> = statement
            .query_map([Id(course)], node_row)?
            .collect::<Result<_, _>>()?;
        let mut children: HashMap<Uuid, Vec<&NodeRecord>> = HashMap::new();
        for node in &nodes {
            if let Some(parent) = node.parent {
                children.entry(parent).or_default().push(node);
            }
        }
        fn section(node: &NodeRecord, children: &HashMap<Uuid, Vec<&NodeRecord>>) -> TreeSection {
            TreeSection {
                id: node.id,
                title: node.title.clone(),
                line: node.line.clone(),
                read: node.read_at.is_some(),
                children: sections(node.id, children),
            }
        }
        fn sections(parent: Uuid, children: &HashMap<Uuid, Vec<&NodeRecord>>) -> Vec<TreeSection> {
            children.get(&parent).map_or_else(Vec::new, |nodes| {
                nodes.iter().map(|node| section(node, children)).collect()
            })
        }
        let chapters = nodes
            .iter()
            .filter(|node| node.parent.is_none() && node.kind == NodeKind::Chapter)
            .map(|chapter| TreeChapter {
                id: chapter.id,
                title: chapter.title.clone(),
                sections: sections(chapter.id, &children),
            })
            .collect();
        Ok(IndexTree { chapters })
    }

    pub fn index_node(&self, node: Uuid) -> Result<Option<NodeRecord>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                &format!(
                    "SELECT {NODE_COLUMNS} FROM index_nodes WHERE id = ?1 AND deleted_at IS NULL"
                ),
                [Id(node)],
                node_row,
            )
            .optional()?)
    }

    /// Records that a section was read now, unless it was read before.
    pub fn mark_read(&self, node: Uuid) -> Result<(), StoreError> {
        let connection = self.connection();
        let found: Option<bool> = connection
            .query_row(
                "SELECT read_at IS NOT NULL FROM index_nodes WHERE id = ?1 AND deleted_at IS NULL",
                [Id(node)],
                |row| row.get(0),
            )
            .optional()?;
        match found {
            None => Err(StoreError::NotFound(format!("section {node}"))),
            Some(true) => Ok(()),
            Some(false) => {
                let changed = connection.execute(
                    "UPDATE index_nodes SET read_at = ?2, updated_at = ?2 WHERE id = ?1",
                    params![Id(node), Time(self.now())],
                )?;
                changed_one(changed, || format!("section {node}"))
            }
        }
    }

    /// The course map: concepts and entries in the order stored.
    pub fn course_map(&self, course: Uuid) -> Result<CourseMap, StoreError> {
        let connection = self.connection();
        let concepts = connection
            .prepare(
                "SELECT name, rung FROM concepts
                 WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY created_at, rowid",
            )?
            .query_map([Id(course)], |row| {
                Ok(Concept {
                    name: row.get(0)?,
                    rung: row.get::<_, Option<u32>>(1)?.map(RungId),
                })
            })?
            .collect::<Result<_, _>>()?;
        let entries = map_entries(&connection, course, None)?;
        Ok(CourseMap { concepts, entries })
    }

    /// The map entry of one node.
    pub fn map_entry(&self, node: Uuid) -> Result<Option<MapEntry>, StoreError> {
        let connection = self.connection();
        let course: Option<Id> = connection
            .query_row(
                "SELECT course_id FROM index_nodes WHERE id = ?1 AND deleted_at IS NULL",
                [Id(node)],
                |row| row.get(0),
            )
            .optional()?;
        let Some(Id(course)) = course else {
            return Ok(None);
        };
        Ok(map_entries(&connection, course, Some(node))?.pop())
    }

    /// Stores a convention unless its key, normalised, is already fixed for
    /// the course and kind; the first fixing is never changed.
    pub fn add_convention(
        &self,
        course: Uuid,
        convention: &Convention,
    ) -> Result<ConventionOutcome, StoreError> {
        let connection = self.connection();
        require_course(&connection, course)?;
        insert_convention(&connection, self.now(), course, convention)
    }

    /// The course's conventions, in the order fixed.
    pub fn conventions(&self, course: Uuid) -> Result<Vec<Convention>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT kind, key, meaning, origin_version_id FROM conventions
             WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY created_at, rowid",
        )?;
        let conventions = statement
            .query_map([Id(course)], |row| {
                Ok(Convention {
                    kind: row.get(0)?,
                    key: row.get(1)?,
                    meaning: row.get(2)?,
                    origin: match optional_id_at(row, 3)? {
                        None => ConventionOrigin::Plan,
                        Some(version) => ConventionOrigin::SectionVersion { version },
                    },
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(conventions)
    }
}
