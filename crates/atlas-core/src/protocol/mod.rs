//! The typed protocol between the core and the interface: one request in, one
//! response out, events on the side.
//!
//! Everything here is exported to TypeScript by ts-rs into
//! `ui/src/core/generated/` (run `npm --prefix ui run gen:types`). Only the four
//! roots below are marked for export; a type is generated when one of them
//! reaches it, so a type the interface must never see (the core's course
//! memory, anything holding a credential) is kept out simply by not appearing
//! here. The data types live beside their feature in the files of this folder.

mod citations;
mod error;
mod export;
mod generation;
mod glossary;
mod index;
mod library;
mod models;
mod placement;
mod sections;
mod session;
mod side_chats;
mod usage;

pub use citations::{Citation, Source};
pub use error::CoreError;
pub use export::{ContentsEntry, ContentsLevel, ExportDocument, ExportResult, ExportSection};
pub use generation::{Activity, GenerationEvent, GenerationOutcome, GenerationTarget};
pub use glossary::{Anchor, GlossaryEntry, WikipediaResult};
pub use index::{ChapterNode, NodeMark, SectionKind, SectionNode};
pub use library::{CourseStage, CourseSummary, CourseView, Progress};
pub use models::{Model, ModelList};
pub use placement::{
    Goal, PlacementAnswer, PlacementQuestion, PlacementState, PlacementStep, SelfRating,
};
pub use sections::{
    Block, BlockContent, BlockKind, CheckQuestion, CheckResult, ExplainStyle, SectionVersion,
    SectionView, VersionStyle, VersionSummary,
};
pub use session::{Account, PlanUsage, Session, SignedOutReason};
pub use side_chats::{ChatMessage, ChatRole, MessageState, SideChat, SideChatSummary};
pub use usage::{UsageFailure, UsageLevel, UsageReading, UsageState, UsageWindow};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// A request from the interface to the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreRequest {
    // Session.
    /// The plan login as it stands.
    GetSession,
    /// Start Sign in with ChatGPT in the system browser.
    SignIn,
    /// Abandon a sign-in waiting for the browser.
    CancelSignIn,
    /// Ask again for permission to use the plan.
    EnablePlanUsage,
    /// End the plan login and the usage meter's login together.
    SignOut,
    /// Open ChatGPT's usage settings in the browser.
    OpenManageUsage,
    /// Open an `http` or `https` link in the browser.
    OpenExternal { url: String },

    // Models.
    /// The models the plan lists, and the one in use.
    ListModels,
    /// Use this model from now on.
    SelectModel { slug: String },

    // Library.
    /// The courses, most recently opened first.
    ListCourses,
    /// Start a course on a topic in the candidate's own words.
    CreateCourse { topic: String },
    /// A course with its index, where it resumes.
    OpenCourse { course_id: Uuid },
    /// Delete a course and everything under it, permanently.
    DeleteCourse { course_id: Uuid },

    // Placement.
    /// The self-rating and the goal that open placement.
    SubmitSelfRating {
        course_id: Uuid,
        rating: SelfRating,
        goal: Goal,
    },
    /// The answer to the placement question shown.
    AnswerPlacement {
        course_id: Uuid,
        question_id: Uuid,
        answer: PlacementAnswer,
    },
    /// Repeat the placement step that failed.
    RetryPlacementStep { course_id: Uuid },

    // Reader.
    /// A section's current version: from the store, or generated and streamed.
    OpenSection { course_id: Uuid, section_id: Uuid },
    /// Stop the generation running for a section.
    StopGeneration { course_id: Uuid, section_id: Uuid },
    /// Generate a section again after a stopped or failed attempt.
    RetrySection { course_id: Uuid, section_id: Uuid },
    /// A new version of a section, written another way.
    ExplainDifferently {
        course_id: Uuid,
        section_id: Uuid,
        style: ExplainStyle,
    },
    /// Make an existing version the current one.
    SelectVersion {
        course_id: Uuid,
        section_id: Uuid,
        version_id: Uuid,
    },
    /// The answer to a check question: `choice` indexes its options.
    AnswerCheck {
        version_id: Uuid,
        block: u32,
        question: u32,
        choice: u32,
    },
    /// What follows `section_id` in the course.
    Next { course_id: Uuid, section_id: Uuid },
    /// Deep dives under a section.
    GoDeeper { course_id: Uuid, section_id: Uuid },

    // Doubts.
    /// A short explanation of the selected text, in place.
    Define { anchor: Anchor, sentence: String },
    /// The Wikipedia summary for a glossary entry's term.
    LookupWikipedia { entry_id: Uuid },
    /// The course's glossary.
    ListGlossary { course_id: Uuid },
    /// The course's side chats, newest first.
    ListSideChats { course_id: Uuid },
    /// A new side chat, on a passage or on the course alone.
    StartSideChat {
        course_id: Uuid,
        anchor: Option<Anchor>,
    },
    /// A side chat with its messages.
    OpenSideChat { chat_id: Uuid },
    /// The candidate's message; the reply streams.
    SendSideChatMessage { chat_id: Uuid, text: String },
    /// Stop the reply streaming in a side chat.
    StopSideChatReply { chat_id: Uuid },

    // Usage.
    /// The usage meter as it stands.
    GetUsage,
    /// Start the usage meter's own sign-in.
    ConnectUsage,
    /// Abandon the usage meter's sign-in.
    CancelConnectUsage,
    /// Whether the window has focus; readings pause without it.
    SetWindowFocused { focused: bool },

    // Export.
    /// The course as it will be printed.
    GetExportDocument { course_id: Uuid },
    /// Write the course to a PDF the candidate chooses.
    ExportPdf { course_id: Uuid },
}

/// The core's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreResponse {
    Session {
        session: Session,
    },
    Models {
        list: ModelList,
    },
    Courses {
        courses: Vec<CourseSummary>,
    },
    Course {
        course: CourseView,
    },
    Placement {
        course_id: Uuid,
        placement: PlacementState,
    },
    Section {
        section: SectionView,
    },
    /// Where Next leads; the interface opens it.
    NextSection {
        course_id: Uuid,
        section_id: Uuid,
    },
    /// Next was asked from the last section of the course.
    EndOfCourse {
        course_id: Uuid,
    },
    /// The deep dives under a section, in order.
    DeepDives {
        course_id: Uuid,
        section_id: Uuid,
        deep_dives: Vec<SectionNode>,
    },
    CheckAnswered {
        result: CheckResult,
    },
    /// The definition, kept in the glossary.
    Definition {
        entry: GlossaryEntry,
    },
    Wikipedia {
        entry_id: Uuid,
        result: WikipediaResult,
    },
    Glossary {
        course_id: Uuid,
        entries: Vec<GlossaryEntry>,
    },
    SideChats {
        course_id: Uuid,
        chats: Vec<SideChatSummary>,
    },
    SideChat {
        chat: SideChat,
    },
    /// The reply to a side-chat message, once it has finished.
    SideChatReply {
        chat_id: Uuid,
        message: ChatMessage,
    },
    Usage {
        usage: UsageState,
    },
    ExportDocument {
        document: ExportDocument,
    },
    Exported {
        result: ExportResult,
    },
    /// The request was carried out and has nothing to return.
    Done,
}

/// What the core reports while it works, outside any response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum CoreEvent {
    SessionChanged {
        session: Session,
    },
    ModelsChanged {
        list: ModelList,
    },
    /// The library changed: a course was added, opened, moved or deleted.
    CoursesChanged {
        courses: Vec<CourseSummary>,
    },
    /// A course's index changed: new nodes, or a node's reading state.
    IndexChanged {
        course_id: Uuid,
        chapters: Vec<ChapterNode>,
    },
    PlacementProgress {
        course_id: Uuid,
        placement: PlacementState,
    },
    /// One step of a generation. Sections, definitions and side-chat replies
    /// share this one stream; `target` says what is being written.
    Generation {
        generation_id: Uuid,
        target: GenerationTarget,
        event: GenerationEvent,
    },
    /// A definition joined the glossary.
    GlossaryChanged {
        course_id: Uuid,
        entry: GlossaryEntry,
    },
    /// A side chat was started or received a message.
    SideChatsChanged {
        course_id: Uuid,
        chat: SideChatSummary,
    },
    /// Up to three prompts offered after a side-chat reply.
    FollowUpPrompts {
        chat_id: Uuid,
        prompts: Vec<String>,
    },
    UsageChanged {
        usage: UsageState,
    },
}
