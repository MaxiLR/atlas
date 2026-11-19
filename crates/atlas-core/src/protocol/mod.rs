//! The typed protocol between the core and the interface.
//!
//! Every type here is exported to TypeScript by ts-rs into
//! `ui/src/core/generated/` (run `npm --prefix ui run gen:types`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// A request from the interface to the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreRequest {
    /// The courses in the library, most recently opened first.
    ListCourses,
    /// Start a course on a topic: the provider builds its index.
    CreateCourse { topic: String },
    /// A course with its index.
    OpenCourse { course_id: Uuid },
    /// A section's content: from the store, or generated and streamed.
    OpenSection { course_id: Uuid, section_id: Uuid },
    /// Open the section that follows `section_id` in the index.
    Next { course_id: Uuid, section_id: Uuid },
}

/// The core's answer to one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreResponse {
    Courses {
        courses: Vec<CourseSummary>,
    },
    Course {
        course: CourseView,
    },
    Section {
        section: SectionView,
    },
    /// `Next` was asked from the last section of the course.
    EndOfCourse {
        course_id: Uuid,
    },
}

/// What the core reports while it works, outside any response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreEvent {
    /// Text appended to a section that is being generated.
    SectionDelta {
        course_id: Uuid,
        section_id: Uuid,
        text: String,
    },
    /// The section finished generating and is stored.
    SectionCompleted { course_id: Uuid, section_id: Uuid },
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreError {
    #[error("{what} not found")]
    NotFound { what: String },
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("the model provider failed: {message}")]
    Provider { message: String },
    #[error("the store failed: {message}")]
    Store { message: String },
    #[error("{feature} is not available yet")]
    Unavailable { feature: String },
}

/// A course as listed in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseSummary {
    pub id: Uuid,
    pub topic: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A course with its index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseView {
    pub id: Uuid,
    pub topic: String,
    pub chapters: Vec<ChapterView>,
}

/// A chapter of the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChapterView {
    pub id: Uuid,
    pub title: String,
    pub sections: Vec<SectionEntry>,
}

/// A section as listed in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SectionEntry {
    pub id: Uuid,
    pub title: String,
    /// One line on what the section covers.
    pub summary: String,
    /// Whether a complete version of the section is stored.
    pub generated: bool,
}

/// A section with its content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SectionView {
    pub id: Uuid,
    pub course_id: Uuid,
    pub title: String,
    pub content: String,
}
