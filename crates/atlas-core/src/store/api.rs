//! Repository API under construction.
#![allow(unused_variables, clippy::too_many_arguments)]

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Store, StoreError};
use crate::domain::conventions::Convention;
use crate::domain::course_map::{Concept, CourseMap, MapEntry};
use crate::domain::ladder::Ladder;
use crate::domain::profile::CandidateProfile;
use crate::domain::questions::{KeyedCheckQuestion, KeyedPlacementQuestion};
use crate::domain::record::StoredRecord;
use crate::index_tree::{IndexTree, TreeSection};
use crate::protocol::{
    Anchor, Block, ChatMessage, ChatRole, Citation, CourseStage, GlossaryEntry, Goal, MessageState,
    PlacementAnswer, SelfRating, Source, VersionStyle, WikipediaResult,
};

pub const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Course,
    LadderRung,
    ProfileRung,
    Misconception,
    PlacementQuestion,
    IndexNode,
    SectionVersion,
    CheckAnswer,
    Concept,
    MapEntry,
    MapEntryConcept,
    Convention,
    GlossaryEntry,
    SideChat,
    ChatMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseRecord {
    pub id: Uuid,
    pub topic: String,
    pub stage: CourseStage,
    pub self_rating: Option<SelfRating>,
    pub goal: Option<Goal>,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub last_read: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementQuestionRecord {
    pub question: KeyedPlacementQuestion,
    pub number: u32,
    pub answer: Option<PlacementAnswer>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Chapter,
    Section,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
    pub id: Uuid,
    pub course_id: Uuid,
    pub parent: Option<Uuid>,
    pub kind: NodeKind,
    pub position: u32,
    pub title: String,
    pub line: String,
    pub read_at: Option<DateTime<Utc>>,
    pub current_version: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckBlock {
    pub block: u32,
    pub questions: Vec<KeyedCheckQuestion>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSectionVersion {
    pub id: Uuid,
    pub node: Uuid,
    pub style: VersionStyle,
    pub blocks: Vec<Block>,
    pub checks: Vec<CheckBlock>,
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
    pub written_without_search: bool,
    pub record: StoredRecord,
    pub model: Option<String>,
    pub prompt: String,
}
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
    pub record: Option<StoredRecord>,
    pub model: Option<String>,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckAnswerRecord {
    pub id: Uuid,
    pub version_id: Uuid,
    pub block: u32,
    pub question: u32,
    pub choice: u32,
    pub correct: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConventionOutcome {
    Stored,
    Rejected,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGlossaryEntry {
    pub course: Uuid,
    pub term: String,
    pub definition: String,
    pub anchor: Anchor,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideChatRecord {
    pub id: Uuid,
    pub course_id: Uuid,
    pub anchor: Option<Anchor>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub state: MessageState,
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
}

type R<T> = Result<T, StoreError>;
impl Store {
    pub fn delete(&self, kind: RecordKind, id: Uuid) -> R<bool> {
        todo!()
    }
    pub fn create_course(&self, topic: &str) -> R<CourseRecord> {
        todo!()
    }
    pub fn course_record(&self, id: Uuid) -> R<Option<CourseRecord>> {
        todo!()
    }
    pub fn course_records(&self) -> R<Vec<CourseRecord>> {
        todo!()
    }
    pub fn set_self_rating(&self, course: Uuid, rating: SelfRating, goal: Goal) -> R<()> {
        todo!()
    }
    pub fn set_stage(&self, course: Uuid, stage: CourseStage) -> R<()> {
        todo!()
    }
    pub fn mark_opened(&self, course: Uuid) -> R<()> {
        todo!()
    }
    pub fn set_last_read(&self, course: Uuid, node: Uuid) -> R<()> {
        todo!()
    }
    pub fn store_ladder(&self, course: Uuid, ladder: &Ladder) -> R<()> {
        todo!()
    }
    pub fn ladder(&self, course: Uuid) -> R<Option<Ladder>> {
        todo!()
    }
    pub fn save_profile(&self, course: Uuid, profile: &CandidateProfile) -> R<()> {
        todo!()
    }
    pub fn profile(&self, course: Uuid) -> R<Option<CandidateProfile>> {
        todo!()
    }
    pub fn add_placement_question(
        &self,
        course: Uuid,
        q: &KeyedPlacementQuestion,
    ) -> R<PlacementQuestionRecord> {
        todo!()
    }
    pub fn answer_placement_question(&self, id: Uuid, a: PlacementAnswer) -> R<()> {
        todo!()
    }
    pub fn placement_questions(&self, course: Uuid) -> R<Vec<PlacementQuestionRecord>> {
        todo!()
    }
    pub fn insert_index(
        &self,
        course: Uuid,
        tree: &IndexTree,
        map: &CourseMap,
        conventions: &[Convention],
    ) -> R<Vec<ConventionOutcome>> {
        todo!()
    }
    pub fn insert_children(
        &self,
        course: Uuid,
        parent: Uuid,
        children: &[TreeSection],
        concepts: &[Concept],
        entries: &[MapEntry],
    ) -> R<()> {
        todo!()
    }
    pub fn index_tree(&self, course: Uuid) -> R<IndexTree> {
        todo!()
    }
    pub fn index_node(&self, id: Uuid) -> R<Option<NodeRecord>> {
        todo!()
    }
    pub fn mark_read(&self, node: Uuid) -> R<()> {
        todo!()
    }
    pub fn course_map(&self, course: Uuid) -> R<CourseMap> {
        todo!()
    }
    pub fn map_entry(&self, node: Uuid) -> R<Option<MapEntry>> {
        todo!()
    }
    pub fn add_convention(&self, course: Uuid, c: &Convention) -> R<ConventionOutcome> {
        todo!()
    }
    pub fn conventions(&self, course: Uuid) -> R<Vec<Convention>> {
        todo!()
    }
    pub fn insert_version(
        &self,
        v: &NewSectionVersion,
        c: &[Convention],
    ) -> R<Vec<ConventionOutcome>> {
        todo!()
    }
    pub fn set_current_version(&self, node: Uuid, version: Uuid) -> R<()> {
        todo!()
    }
    pub fn version(&self, id: Uuid) -> R<Option<StoredSectionVersion>> {
        todo!()
    }
    pub fn versions(&self, node: Uuid) -> R<Vec<StoredSectionVersion>> {
        todo!()
    }
    pub fn current_version(&self, node: Uuid) -> R<Option<StoredSectionVersion>> {
        todo!()
    }
    pub fn current_record(&self, node: Uuid) -> R<Option<StoredRecord>> {
        todo!()
    }
    pub fn add_check_answer(
        &self,
        v: Uuid,
        block: u32,
        question: u32,
        choice: u32,
        correct: bool,
    ) -> R<CheckAnswerRecord> {
        todo!()
    }
    pub fn check_answer(&self, v: Uuid, block: u32, question: u32) -> R<Option<CheckAnswerRecord>> {
        todo!()
    }
    pub fn check_answers(&self, v: Uuid) -> R<Vec<CheckAnswerRecord>> {
        todo!()
    }
    pub fn add_glossary_entry(&self, e: &NewGlossaryEntry) -> R<GlossaryEntry> {
        todo!()
    }
    pub fn glossary(&self, course: Uuid) -> R<Vec<GlossaryEntry>> {
        todo!()
    }
    pub fn glossary_entry(&self, id: Uuid) -> R<Option<GlossaryEntry>> {
        todo!()
    }
    pub fn glossary_entry_for_anchor(&self, a: &Anchor) -> R<Option<GlossaryEntry>> {
        todo!()
    }
    pub fn glossary_for_version(&self, v: Uuid) -> R<Vec<GlossaryEntry>> {
        todo!()
    }
    pub fn set_wikipedia(&self, id: Uuid, w: &WikipediaResult) -> R<()> {
        todo!()
    }
    pub fn start_side_chat(&self, course: Uuid, a: Option<&Anchor>) -> R<SideChatRecord> {
        todo!()
    }
    pub fn side_chat(&self, id: Uuid) -> R<Option<SideChatRecord>> {
        todo!()
    }
    pub fn side_chats(&self, course: Uuid) -> R<Vec<SideChatRecord>> {
        todo!()
    }
    pub fn side_chats_for_version(&self, v: Uuid) -> R<Vec<SideChatRecord>> {
        todo!()
    }
    pub fn add_chat_message(&self, chat: Uuid, m: &NewChatMessage) -> R<ChatMessage> {
        todo!()
    }
    pub fn chat_messages(&self, chat: Uuid) -> R<Vec<ChatMessage>> {
        todo!()
    }
    pub fn selected_model(&self) -> R<Option<String>> {
        todo!()
    }
    pub fn set_selected_model(&self, slug: Option<&str>) -> R<()> {
        todo!()
    }
    pub fn host_id(&self) -> R<Uuid> {
        todo!()
    }
}
