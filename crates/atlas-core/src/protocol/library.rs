use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{ChapterNode, PlacementState};

/// Where a course stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum CourseStage {
    /// Placement is under way; the index does not exist yet.
    Placement,
    /// The index exists; the course is read.
    Ready,
}

/// Sections read out of the sections present (broad pass and deep dives).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub read: u32,
    pub total: u32,
}

/// A course as listed in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CourseSummary {
    pub id: Uuid,
    pub topic: String,
    pub stage: CourseStage,
    pub progress: Progress,
    /// The title of the section the course resumes at.
    pub resume_title: Option<String>,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A course with its index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CourseView {
    pub id: Uuid,
    pub topic: String,
    pub stage: CourseStage,
    /// Empty while the course is in placement.
    pub chapters: Vec<ChapterNode>,
    /// The section to open: the one last read, or the first.
    pub resume_at: Option<Uuid>,
    pub glossary_count: u32,
    pub side_chat_count: u32,
    /// Where placement stands, while the course is in placement.
    pub placement: Option<PlacementState>,
}
