//! Side chats, anchored to a passage of one version or to the course only,
//! and their messages.

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, Row, params};
use uuid::Uuid;

use super::courses::{ERASE_CHAT_MESSAGE, ERASE_SIDE_CHAT, require_course};
use super::{Id, Store, StoreError, Time, changed_one, id_at, json, json_at, time_at};
use crate::protocol::{Anchor, ChatMessage, ChatRole, Citation, MessageState, Source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideChatRecord {
    pub id: Uuid,
    pub course_id: Uuid,
    pub anchor: Option<Anchor>,
    pub created_at: DateTime<Utc>,
    /// Moves with every message.
    pub updated_at: DateTime<Utc>,
}

/// A message to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub state: MessageState,
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
}

const CHAT_COLUMNS: &str = "id, course_id, version_id, block, quote, created_at, updated_at";

fn chat_row(row: &Row<'_>) -> rusqlite::Result<SideChatRecord> {
    let anchor = match row.get::<_, Option<Id>>(2)? {
        None => None,
        Some(Id(version_id)) => Some(Anchor {
            version_id,
            block: row.get(3)?,
            quote: row.get(4)?,
        }),
    };
    Ok(SideChatRecord {
        id: id_at(row, 0)?,
        course_id: id_at(row, 1)?,
        anchor,
        created_at: time_at(row, 5)?,
        updated_at: time_at(row, 6)?,
    })
}

fn message_row(row: &Row<'_>) -> rusqlite::Result<ChatMessage> {
    Ok(ChatMessage {
        id: id_at(row, 0)?,
        role: row.get(1)?,
        text: row.get(2)?,
        state: row.get(3)?,
        citations: json_at(row, 4)?,
        sources: json_at(row, 5)?,
        created_at: time_at(row, 6)?,
    })
}

impl Store {
    /// Starts a side chat, anchored to a passage or not.
    pub fn start_side_chat(
        &self,
        course: Uuid,
        anchor: Option<&Anchor>,
    ) -> Result<SideChatRecord, StoreError> {
        let now = self.now();
        let id = Uuid::new_v4();
        let connection = self.connection();
        require_course(&connection, course)?;
        connection.execute(
            "INSERT INTO side_chats
                (id, course_id, version_id, block, quote, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                Id(id),
                Id(course),
                anchor.map(|anchor| Id(anchor.version_id)),
                anchor.map(|anchor| anchor.block),
                anchor.map(|anchor| anchor.quote.as_str()),
                Time(now)
            ],
        )?;
        Ok(SideChatRecord {
            id,
            course_id: course,
            anchor: anchor.cloned(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn side_chat(&self, chat: Uuid) -> Result<Option<SideChatRecord>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                &format!(
                    "SELECT {CHAT_COLUMNS} FROM side_chats WHERE id = ?1 AND deleted_at IS NULL"
                ),
                [Id(chat)],
                chat_row,
            )
            .optional()?)
    }

    /// The course's side chats, newest first.
    pub fn side_chats(&self, course: Uuid) -> Result<Vec<SideChatRecord>, StoreError> {
        self.side_chats_where("course_id = ?1", course)
    }

    /// The side chats anchored to one version, which show only on it.
    pub fn side_chats_for_version(&self, version: Uuid) -> Result<Vec<SideChatRecord>, StoreError> {
        self.side_chats_where("version_id = ?1", version)
    }

    fn side_chats_where(
        &self,
        condition: &str,
        id: Uuid,
    ) -> Result<Vec<SideChatRecord>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {CHAT_COLUMNS} FROM side_chats WHERE {condition} AND deleted_at IS NULL
             ORDER BY created_at DESC, rowid DESC"
        ))?;
        let chats = statement
            .query_map([Id(id)], chat_row)?
            .collect::<Result<_, _>>()?;
        Ok(chats)
    }

    /// Adds a message to a chat and moves the chat's update time, in one
    /// transaction.
    pub fn add_chat_message(
        &self,
        chat: Uuid,
        message: &NewChatMessage,
    ) -> Result<ChatMessage, StoreError> {
        let now = self.now();
        let id = Uuid::new_v4();
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE side_chats SET updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![Id(chat), Time(now)],
        )?;
        changed_one(changed, || format!("side chat {chat}"))?;
        transaction.execute(
            "INSERT INTO side_chat_messages
                (id, chat_id, role, text, state, citations, sources, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                Id(id),
                Id(chat),
                message.role,
                message.text,
                message.state,
                json(&message.citations)?,
                json(&message.sources)?,
                Time(now)
            ],
        )?;
        transaction.commit()?;
        Ok(ChatMessage {
            id,
            role: message.role,
            text: message.text.clone(),
            state: message.state,
            citations: message.citations.clone(),
            sources: message.sources.clone(),
            created_at: now,
        })
    }

    /// A chat's thread, oldest first.
    pub fn chat_messages(&self, chat: Uuid) -> Result<Vec<ChatMessage>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, role, text, state, citations, sources, created_at
             FROM side_chat_messages
             WHERE chat_id = ?1 AND deleted_at IS NULL ORDER BY created_at, rowid",
        )?;
        let messages = statement
            .query_map([Id(chat)], message_row)?
            .collect::<Result<_, _>>()?;
        Ok(messages)
    }

    /// Deletes a live chat and its messages permanently, in one transaction:
    /// every row stays as a tombstone with its content erased, as a course's
    /// deletion leaves it; a message deleted before keeps its deletion time.
    /// An absent or already deleted chat is not found, and nothing changes.
    pub fn delete_side_chat(&self, chat: Uuid) -> Result<(), StoreError> {
        let now = Time(self.now());
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            &format!(
                "UPDATE side_chats SET {ERASE_SIDE_CHAT}
                     deleted_at = ?2, updated_at = ?2
                 WHERE id = ?1 AND deleted_at IS NULL"
            ),
            params![Id(chat), now],
        )?;
        changed_one(changed, || format!("side chat {chat}"))?;
        transaction.execute(
            &format!(
                "UPDATE side_chat_messages SET {ERASE_CHAT_MESSAGE}
                     deleted_at = COALESCE(deleted_at, ?2), updated_at = ?2
                 WHERE chat_id = ?1"
            ),
            params![Id(chat), now],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
