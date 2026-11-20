use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{Anchor, Citation, Source};

/// A side chat as listed in the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SideChatSummary {
    pub id: Uuid,
    pub course_id: Uuid,
    /// The passage it is anchored to, if any.
    pub anchor: Option<Anchor>,
    pub section_title: Option<String>,
    /// The passage, or without one the first question.
    pub opening: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A side chat with its thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SideChat {
    pub id: Uuid,
    pub course_id: Uuid,
    pub anchor: Option<Anchor>,
    pub section_title: Option<String>,
    pub messages: Vec<ChatMessage>,
    /// Offered after the last reply; at most three.
    pub follow_ups: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ChatRole {
    Candidate,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum MessageState {
    Complete,
    /// A reply stopped before it finished; what arrived is kept.
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: ChatRole,
    pub text: String,
    pub state: MessageState,
    /// Ranges of `text`; their `block` is 0.
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
    pub created_at: DateTime<Utc>,
}
