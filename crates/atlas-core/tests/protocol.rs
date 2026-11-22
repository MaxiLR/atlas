//! The protocol between the core and the interface: stable tag names, JSON
//! round trips, `Unavailable` for features not built yet, and the bindings
//! generated for the interface, which reach no core-only and no token-bearing
//! type.

mod support;

use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::domain::record::SectionRecord;
use atlas_core::protocol::{
    Account, Activity, Anchor, Block, BlockContent, BlockKind, ChapterNode, ChatMessage, ChatRole,
    CheckQuestion, CheckResult, Citation, ContentsEntry, ContentsLevel, CoreError, CoreEvent,
    CoreRequest, CoreResponse, CourseStage, CourseSummary, CourseView, ExplainStyle,
    ExportDocument, ExportResult, ExportSection, GenerationEvent, GenerationOutcome,
    GenerationTarget, GlossaryEntry, Goal, MessageState, Model, ModelList, NodeMark,
    PlacementAnswer, PlacementQuestion, PlacementState, PlacementStep, PlanUsage, Progress,
    SectionKind, SectionNode, SectionVersion, SectionView, SelfRating, Session, SideChat,
    SideChatSummary, SignedOutReason, Source, UsageFailure, UsageLevel, UsageReading, UsageState,
    UsageWindow, VersionStyle, VersionSummary, WikipediaResult,
};
use atlas_core::testing::{FakePorts, FakeProvider, FakeScript, RecordingEvents};
use atlas_core::{Core, CoreDeps};
use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use ts_rs::{Config, TS, TypeVisitor};
use uuid::Uuid;

use support::sample_course::{self, sample_course};
use support::server::{Reply, Route, TestServer};

// -- values ------------------------------------------------------------------

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 19, 9, 30, 0).unwrap()
}

fn anchor() -> Anchor {
    Anchor {
        version_id: id(0x1101),
        block: 0,
        quote: "mantle".to_owned(),
    }
}

fn section_node(read: bool) -> SectionNode {
    SectionNode {
        id: id(0x101),
        kind: SectionKind::BroadPass,
        title: "Layers of the Earth".to_owned(),
        summary: "Crust, mantle and core.".to_owned(),
        read,
        mark: if read {
            NodeMark::Read
        } else {
            NodeMark::Unread
        },
        deep_dives: vec![SectionNode {
            id: id(0x111),
            kind: SectionKind::DeepDive,
            title: "The core".to_owned(),
            summary: "Iron at the centre.".to_owned(),
            read: false,
            mark: NodeMark::Unread,
            deep_dives: Vec::new(),
        }],
    }
}

fn chapters() -> Vec<ChapterNode> {
    vec![ChapterNode {
        id: id(0x100),
        title: "Earth's structure".to_owned(),
        sections: vec![section_node(true)],
    }]
}

fn citation() -> Citation {
    Citation {
        block: 0,
        start: 4,
        end: 9,
        source: 1,
    }
}

fn source() -> Source {
    Source {
        number: 1,
        url: "https://en.wikipedia.org/wiki/Mantle_(geology)".to_owned(),
        title: "Mantle (geology)".to_owned(),
    }
}

fn blocks() -> Vec<Block> {
    let description = Some("A cross-section of the Earth.".to_owned());
    let contents = vec![
        BlockContent::Prose {
            text: "The mantle is solid rock that flows.".to_owned(),
        },
        BlockContent::Visual {
            spec: json!({"root": "callout", "elements": {"callout": {"type": "Callout"}}}),
            description: description.clone(),
        },
        BlockContent::Figure {
            svg: serde_json::from_value(json!("<svg viewBox=\"0 0 10 10\"></svg>")).unwrap(),
            description: description.clone(),
        },
        BlockContent::Mermaid {
            source: "graph TD; crust-->mantle".to_owned(),
            description: description.clone(),
        },
        BlockContent::Check {
            questions: vec![
                CheckQuestion {
                    index: 0,
                    stem: "What happens next?".to_owned(),
                    options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                    answer: None,
                },
                CheckQuestion {
                    index: 1,
                    stem: "What do you do first?".to_owned(),
                    options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                    answer: Some(check_result()),
                },
            ],
        },
        BlockContent::Fallback {
            description: "A cross-section of the Earth.".to_owned(),
        },
    ];
    (0..)
        .zip(contents)
        .map(|(index, content)| Block { index, content })
        .collect()
}

fn check_result() -> CheckResult {
    CheckResult {
        choice: 2,
        correct: false,
        key: 1,
        explanation: "The mantle is solid.".to_owned(),
    }
}

fn version() -> SectionVersion {
    SectionVersion {
        id: id(0x1101),
        style: VersionStyle::Original,
        blocks: blocks(),
        citations: vec![citation()],
        sources: vec![source()],
        written_without_search: false,
        model: Some("gpt-test".to_owned()),
        created_at: time(),
    }
}

fn section_view() -> SectionView {
    SectionView {
        id: id(0x101),
        course_id: id(1),
        title: "Layers of the Earth".to_owned(),
        summary: "Crust, mantle and core.".to_owned(),
        version: version(),
        versions: vec![VersionSummary {
            id: id(0x1101),
            style: VersionStyle::Original,
            created_at: time(),
        }],
    }
}

fn placement_question() -> PlacementQuestion {
    PlacementQuestion {
        id: id(0x50),
        number: 2,
        stem: "A plate moves 5 cm a year. What happens to the ocean between?".to_owned(),
        options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
    }
}

fn placement(step: PlacementStep) -> PlacementState {
    PlacementState {
        step,
        min_questions: 4,
        max_questions: 6,
        error: None,
    }
}

fn course_summary() -> CourseSummary {
    CourseSummary {
        id: id(1),
        topic: "Plate tectonics".to_owned(),
        stage: CourseStage::Ready,
        progress: Progress { read: 1, total: 6 },
        resume_title: Some("Layers of the Earth".to_owned()),
        last_opened_at: Some(time()),
        created_at: time(),
        updated_at: time(),
    }
}

fn course_view() -> CourseView {
    CourseView {
        id: id(1),
        topic: "Plate tectonics".to_owned(),
        stage: CourseStage::Placement,
        chapters: chapters(),
        resume_at: Some(id(0x101)),
        glossary_count: 1,
        side_chat_count: 2,
        placement: Some(placement(PlacementStep::Question {
            question: placement_question(),
        })),
    }
}

fn glossary_entry() -> GlossaryEntry {
    GlossaryEntry {
        id: id(0x70),
        course_id: id(1),
        section_id: id(0x101),
        section_title: "Layers of the Earth".to_owned(),
        term: "mantle".to_owned(),
        definition: "The thick layer of hot rock under the crust.".to_owned(),
        anchor: anchor(),
        wikipedia: Some(WikipediaResult::Found {
            title: "Mantle (geology)".to_owned(),
            extract: "A mantle is a layer inside a planetary body.".to_owned(),
            url: "https://en.wikipedia.org/wiki/Mantle_(geology)".to_owned(),
        }),
        created_at: time(),
    }
}

fn chat_summary() -> SideChatSummary {
    SideChatSummary {
        id: id(0x80),
        course_id: id(1),
        anchor: Some(anchor()),
        section_title: Some("Layers of the Earth".to_owned()),
        opening: "mantle".to_owned(),
        created_at: time(),
        updated_at: time(),
    }
}

fn chat_message(role: ChatRole, state: MessageState) -> ChatMessage {
    ChatMessage {
        id: id(0x81),
        role,
        text: "Why does solid rock flow?".to_owned(),
        state,
        citations: vec![citation()],
        sources: vec![source()],
        created_at: time(),
    }
}

fn side_chat() -> SideChat {
    SideChat {
        id: id(0x80),
        course_id: id(1),
        anchor: Some(anchor()),
        section_title: Some("Layers of the Earth".to_owned()),
        messages: vec![
            chat_message(ChatRole::Candidate, MessageState::Complete),
            chat_message(ChatRole::Agent, MessageState::Stopped),
        ],
        follow_ups: vec!["How fast?".to_owned()],
        created_at: time(),
        updated_at: time(),
    }
}

fn usage_reading(stale: Option<UsageFailure>) -> UsageReading {
    UsageReading {
        windows: vec![
            UsageWindow {
                name: "5h".to_owned(),
                length_seconds: 18_000,
                used: 0.38,
                resets_at: Some(time()),
                level: UsageLevel::Normal,
            },
            UsageWindow {
                name: "7d".to_owned(),
                length_seconds: 604_800,
                used: 0.92,
                resets_at: None,
                level: UsageLevel::Critical,
            },
        ],
        taken_at: time(),
        stale,
    }
}

fn export_document() -> ExportDocument {
    ExportDocument {
        course_id: id(1),
        title: "Plate tectonics".to_owned(),
        contents: vec![
            ContentsEntry {
                node_id: id(0x100),
                level: ContentsLevel::Chapter,
                depth: 0,
                title: "Earth's structure".to_owned(),
                included: true,
            },
            ContentsEntry {
                node_id: id(0x101),
                level: ContentsLevel::Section,
                depth: 1,
                title: "Layers of the Earth".to_owned(),
                included: true,
            },
            ContentsEntry {
                node_id: id(0x111),
                level: ContentsLevel::DeepDive,
                depth: 2,
                title: "The core".to_owned(),
                included: false,
            },
        ],
        sections: vec![ExportSection {
            node_id: id(0x101),
            kind: SectionKind::BroadPass,
            depth: 1,
            title: "Layers of the Earth".to_owned(),
            summary: "Crust, mantle and core.".to_owned(),
            version: version(),
        }],
        glossary: vec![glossary_entry()],
    }
}

// -- round trips ---------------------------------------------------------------

/// Serialises `value`, checks its tag, and reads it back unchanged.
fn assert_tagged_round_trip<T>(value: &T, tag: &str)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = serde_json::to_value(value).unwrap();
    assert_eq!(json["type"], json!(tag), "the tag of {json}");
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, value, "round trip of {json}");
}

/// For a fieldless enum: serialises to the bare name and reads it back.
fn assert_name_round_trip<T>(value: &T, name: &str)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = serde_json::to_value(value).unwrap();
    assert_eq!(json, json!(name));
    let back: T = serde_json::from_value(json).unwrap();
    assert_eq!(&back, value);
}

/// The tag of every request, in declaration order. Adding a request without
/// naming it here fails to compile.
fn request_tag(request: &CoreRequest) -> &'static str {
    match request {
        CoreRequest::GetSession => "GetSession",
        CoreRequest::SignIn => "SignIn",
        CoreRequest::CancelSignIn => "CancelSignIn",
        CoreRequest::EnablePlanUsage => "EnablePlanUsage",
        CoreRequest::SignOut => "SignOut",
        CoreRequest::OpenManageUsage => "OpenManageUsage",
        CoreRequest::OpenExternal { .. } => "OpenExternal",
        CoreRequest::ListModels => "ListModels",
        CoreRequest::SelectModel { .. } => "SelectModel",
        CoreRequest::ListCourses => "ListCourses",
        CoreRequest::CreateCourse { .. } => "CreateCourse",
        CoreRequest::OpenCourse { .. } => "OpenCourse",
        CoreRequest::DeleteCourse { .. } => "DeleteCourse",
        CoreRequest::SubmitSelfRating { .. } => "SubmitSelfRating",
        CoreRequest::AnswerPlacement { .. } => "AnswerPlacement",
        CoreRequest::RetryPlacementStep { .. } => "RetryPlacementStep",
        CoreRequest::OpenSection { .. } => "OpenSection",
        CoreRequest::StopGeneration { .. } => "StopGeneration",
        CoreRequest::RetrySection { .. } => "RetrySection",
        CoreRequest::ExplainDifferently { .. } => "ExplainDifferently",
        CoreRequest::SelectVersion { .. } => "SelectVersion",
        CoreRequest::AnswerCheck { .. } => "AnswerCheck",
        CoreRequest::Next { .. } => "Next",
        CoreRequest::GoDeeper { .. } => "GoDeeper",
        CoreRequest::Define { .. } => "Define",
        CoreRequest::LookupWikipedia { .. } => "LookupWikipedia",
        CoreRequest::ListGlossary { .. } => "ListGlossary",
        CoreRequest::ListSideChats { .. } => "ListSideChats",
        CoreRequest::StartSideChat { .. } => "StartSideChat",
        CoreRequest::OpenSideChat { .. } => "OpenSideChat",
        CoreRequest::SendSideChatMessage { .. } => "SendSideChatMessage",
        CoreRequest::StopSideChatReply { .. } => "StopSideChatReply",
        CoreRequest::DeleteGlossaryEntry { .. } => "DeleteGlossaryEntry",
        CoreRequest::DeleteSideChat { .. } => "DeleteSideChat",
        CoreRequest::GetUsage => "GetUsage",
        CoreRequest::SetWindowFocused { .. } => "SetWindowFocused",
        CoreRequest::GetExportDocument { .. } => "GetExportDocument",
        CoreRequest::ExportPdf { .. } => "ExportPdf",
    }
}

/// One value of every request, with the feature that answers it.
fn every_request() -> Vec<(CoreRequest, &'static str)> {
    let course_id = id(1);
    let section_id = id(0x101);
    vec![
        (CoreRequest::GetSession, "session"),
        (CoreRequest::SignIn, "session"),
        (CoreRequest::CancelSignIn, "session"),
        (CoreRequest::EnablePlanUsage, "session"),
        (CoreRequest::SignOut, "session"),
        (CoreRequest::OpenManageUsage, "external"),
        (
            CoreRequest::OpenExternal {
                url: "https://example.com/".to_owned(),
            },
            "external",
        ),
        (CoreRequest::ListModels, "models"),
        (
            CoreRequest::SelectModel {
                slug: "gpt-test".to_owned(),
            },
            "models",
        ),
        (CoreRequest::ListCourses, "library"),
        (
            CoreRequest::CreateCourse {
                topic: "Plate tectonics".to_owned(),
            },
            "placement",
        ),
        (CoreRequest::OpenCourse { course_id }, "library"),
        (CoreRequest::DeleteCourse { course_id }, "library"),
        (
            CoreRequest::SubmitSelfRating {
                course_id,
                rating: SelfRating::KnowTheBasics,
                goal: Goal::WorkingKnowledge,
            },
            "placement",
        ),
        (
            CoreRequest::AnswerPlacement {
                course_id,
                question_id: id(0x50),
                answer: PlacementAnswer::Option { index: 2 },
            },
            "placement",
        ),
        (CoreRequest::RetryPlacementStep { course_id }, "placement"),
        (
            CoreRequest::OpenSection {
                course_id,
                section_id,
            },
            "sections",
        ),
        (
            CoreRequest::StopGeneration {
                course_id,
                section_id,
            },
            "sections",
        ),
        (
            CoreRequest::RetrySection {
                course_id,
                section_id,
            },
            "sections",
        ),
        (
            CoreRequest::ExplainDifferently {
                course_id,
                section_id,
                style: ExplainStyle::MoreExamples,
            },
            "versions",
        ),
        (
            CoreRequest::SelectVersion {
                course_id,
                section_id,
                version_id: id(0x1101),
            },
            "versions",
        ),
        (
            CoreRequest::AnswerCheck {
                version_id: id(0x1101),
                block: 4,
                question: 0,
                choice: 1,
            },
            "checks",
        ),
        (
            CoreRequest::Next {
                course_id,
                section_id,
            },
            "navigation",
        ),
        (
            CoreRequest::GoDeeper {
                course_id,
                section_id,
            },
            "deep_dives",
        ),
        (
            CoreRequest::Define {
                anchor: anchor(),
                sentence: "The mantle is solid rock that flows.".to_owned(),
            },
            "define",
        ),
        (
            CoreRequest::LookupWikipedia { entry_id: id(0x70) },
            "wikipedia",
        ),
        (CoreRequest::ListGlossary { course_id }, "define"),
        (CoreRequest::ListSideChats { course_id }, "side_chats"),
        (
            CoreRequest::StartSideChat {
                course_id,
                anchor: Some(anchor()),
            },
            "side_chats",
        ),
        (
            CoreRequest::OpenSideChat { chat_id: id(0x80) },
            "side_chats",
        ),
        (
            CoreRequest::SendSideChatMessage {
                chat_id: id(0x80),
                text: "Why does solid rock flow?".to_owned(),
            },
            "side_chats",
        ),
        (
            CoreRequest::StopSideChatReply { chat_id: id(0x80) },
            "side_chats",
        ),
        (
            CoreRequest::DeleteGlossaryEntry { entry_id: id(0x70) },
            "define",
        ),
        (
            CoreRequest::DeleteSideChat { chat_id: id(0x80) },
            "side_chats",
        ),
        (CoreRequest::GetUsage, "usage"),
        (CoreRequest::SetWindowFocused { focused: true }, "usage"),
        (CoreRequest::GetExportDocument { course_id }, "export"),
        (CoreRequest::ExportPdf { course_id }, "export"),
    ]
}

#[test]
fn every_request_round_trips_with_its_stable_tag() {
    let expected = [
        "GetSession",
        "SignIn",
        "CancelSignIn",
        "EnablePlanUsage",
        "SignOut",
        "OpenManageUsage",
        "OpenExternal",
        "ListModels",
        "SelectModel",
        "ListCourses",
        "CreateCourse",
        "OpenCourse",
        "DeleteCourse",
        "SubmitSelfRating",
        "AnswerPlacement",
        "RetryPlacementStep",
        "OpenSection",
        "StopGeneration",
        "RetrySection",
        "ExplainDifferently",
        "SelectVersion",
        "AnswerCheck",
        "Next",
        "GoDeeper",
        "Define",
        "LookupWikipedia",
        "ListGlossary",
        "ListSideChats",
        "StartSideChat",
        "OpenSideChat",
        "SendSideChatMessage",
        "StopSideChatReply",
        "DeleteGlossaryEntry",
        "DeleteSideChat",
        "GetUsage",
        "SetWindowFocused",
        "GetExportDocument",
        "ExportPdf",
    ];
    let requests = every_request();
    let tags: Vec<_> = requests
        .iter()
        .map(|(request, _)| request_tag(request))
        .collect();
    assert_eq!(tags, expected);
    for (request, _) in &requests {
        assert_tagged_round_trip(request, request_tag(request));
    }
    // Nested values carried by requests.
    assert_tagged_round_trip(&PlacementAnswer::Option { index: 3 }, "Option");
    assert_tagged_round_trip(&PlacementAnswer::NotSure, "NotSure");
    for (rating, name) in [
        (SelfRating::NewToThis, "NewToThis"),
        (SelfRating::KnowTheBasics, "KnowTheBasics"),
        (SelfRating::UseItRegularly, "UseItRegularly"),
        (SelfRating::Expert, "Expert"),
    ] {
        assert_name_round_trip(&rating, name);
    }
    for (goal, name) in [
        (Goal::Overview, "Overview"),
        (Goal::WorkingKnowledge, "WorkingKnowledge"),
        (Goal::Expert, "Expert"),
    ] {
        assert_name_round_trip(&goal, name);
    }
    for (style, name) in [
        (ExplainStyle::Simpler, "Simpler"),
        (ExplainStyle::MoreTechnical, "MoreTechnical"),
        (ExplainStyle::MoreExamples, "MoreExamples"),
    ] {
        assert_name_round_trip(&style, name);
    }
}

#[test]
fn fields_travel_in_camel_case() {
    assert_eq!(
        serde_json::to_value(CoreRequest::OpenSection {
            course_id: id(1),
            section_id: id(2),
        })
        .unwrap(),
        json!({
            "type": "OpenSection",
            "courseId": id(1).to_string(),
            "sectionId": id(2).to_string(),
        })
    );
    assert_eq!(
        serde_json::to_value(CoreEvent::Generation {
            generation_id: id(9),
            target: GenerationTarget::Section {
                course_id: id(1),
                section_id: id(2),
                style: VersionStyle::Original,
            },
            event: GenerationEvent::TextDelta {
                block: 0,
                text: "The crust ".to_owned(),
            },
        })
        .unwrap(),
        json!({
            "type": "Generation",
            "generationId": id(9).to_string(),
            "target": {
                "type": "Section",
                "courseId": id(1).to_string(),
                "sectionId": id(2).to_string(),
                "style": "Original",
            },
            "event": {"type": "TextDelta", "block": 0, "text": "The crust "},
        })
    );
    assert_eq!(
        serde_json::to_value(CoreError::PlanLimit {
            resets_at: Some(time()),
        })
        .unwrap(),
        json!({"type": "PlanLimit", "resetsAt": "2026-11-19T09:30:00Z"})
    );
}

fn response_tag(response: &CoreResponse) -> &'static str {
    match response {
        CoreResponse::Session { .. } => "Session",
        CoreResponse::Models { .. } => "Models",
        CoreResponse::Courses { .. } => "Courses",
        CoreResponse::Course { .. } => "Course",
        CoreResponse::Placement { .. } => "Placement",
        CoreResponse::Section { .. } => "Section",
        CoreResponse::NextSection { .. } => "NextSection",
        CoreResponse::EndOfCourse { .. } => "EndOfCourse",
        CoreResponse::DeepDives { .. } => "DeepDives",
        CoreResponse::CheckAnswered { .. } => "CheckAnswered",
        CoreResponse::Definition { .. } => "Definition",
        CoreResponse::Wikipedia { .. } => "Wikipedia",
        CoreResponse::Glossary { .. } => "Glossary",
        CoreResponse::SideChats { .. } => "SideChats",
        CoreResponse::SideChat { .. } => "SideChat",
        CoreResponse::SideChatReply { .. } => "SideChatReply",
        CoreResponse::Usage { .. } => "Usage",
        CoreResponse::ExportDocument { .. } => "ExportDocument",
        CoreResponse::Exported { .. } => "Exported",
        CoreResponse::Done => "Done",
    }
}

#[test]
fn every_response_round_trips_with_its_stable_tag() {
    let responses = vec![
        CoreResponse::Session {
            session: Session::SignedIn {
                account: Account {
                    email: "candidate@example.com".to_owned(),
                },
                plan_usage: PlanUsage::On,
            },
        },
        CoreResponse::Models {
            list: ModelList {
                models: vec![Model {
                    slug: "gpt-test".to_owned(),
                    display_name: "GPT Test".to_owned(),
                }],
                current: Some("gpt-test".to_owned()),
            },
        },
        CoreResponse::Courses {
            courses: vec![course_summary()],
        },
        CoreResponse::Course {
            course: course_view(),
        },
        CoreResponse::Placement {
            course_id: id(1),
            placement: placement(PlacementStep::PreparingQuestions),
        },
        CoreResponse::Section {
            section: section_view(),
        },
        CoreResponse::NextSection {
            course_id: id(1),
            section_id: id(0x102),
        },
        CoreResponse::EndOfCourse { course_id: id(1) },
        CoreResponse::DeepDives {
            course_id: id(1),
            section_id: id(0x101),
            deep_dives: vec![section_node(false)],
        },
        CoreResponse::CheckAnswered {
            result: check_result(),
        },
        CoreResponse::Definition {
            entry: glossary_entry(),
        },
        CoreResponse::Wikipedia {
            entry_id: id(0x70),
            result: WikipediaResult::NoArticle,
        },
        CoreResponse::Glossary {
            course_id: id(1),
            entries: vec![glossary_entry()],
        },
        CoreResponse::SideChats {
            course_id: id(1),
            chats: vec![chat_summary()],
        },
        CoreResponse::SideChat { chat: side_chat() },
        CoreResponse::SideChatReply {
            chat_id: id(0x80),
            message: chat_message(ChatRole::Agent, MessageState::Complete),
        },
        CoreResponse::Usage {
            usage: UsageState::Connected {
                reading: usage_reading(None),
            },
        },
        CoreResponse::ExportDocument {
            document: export_document(),
        },
        CoreResponse::Exported {
            result: ExportResult::Saved {
                path: "/tmp/Plate tectonics.pdf".to_owned(),
            },
        },
        CoreResponse::Done,
    ];
    let tags: Vec<_> = responses.iter().map(response_tag).collect();
    assert_eq!(
        tags,
        [
            "Session",
            "Models",
            "Courses",
            "Course",
            "Placement",
            "Section",
            "NextSection",
            "EndOfCourse",
            "DeepDives",
            "CheckAnswered",
            "Definition",
            "Wikipedia",
            "Glossary",
            "SideChats",
            "SideChat",
            "SideChatReply",
            "Usage",
            "ExportDocument",
            "Exported",
            "Done",
        ]
    );
    for response in &responses {
        assert_tagged_round_trip(response, response_tag(response));
    }
}

#[test]
fn every_value_the_responses_carry_round_trips() {
    for (session, tag) in [
        (Session::SignedOut { reason: None }, "SignedOut"),
        (Session::SigningIn, "SigningIn"),
        (
            Session::SignedIn {
                account: Account {
                    email: "candidate@example.com".to_owned(),
                },
                plan_usage: PlanUsage::NotAllowed,
            },
            "SignedIn",
        ),
    ] {
        assert_tagged_round_trip(&session, tag);
    }
    for (reason, tag) in [
        (SignedOutReason::SignedOut, "SignedOut"),
        (SignedOutReason::LoginEnded, "LoginEnded"),
        (SignedOutReason::Declined, "Declined"),
        (
            SignedOutReason::Failed {
                error: CoreError::Offline,
            },
            "Failed",
        ),
    ] {
        assert_tagged_round_trip(&reason, tag);
        assert_tagged_round_trip(
            &Session::SignedOut {
                reason: Some(reason),
            },
            "SignedOut",
        );
    }
    for (usage, name) in [
        (PlanUsage::On, "On"),
        (PlanUsage::Off, "Off"),
        (PlanUsage::NotAllowed, "NotAllowed"),
    ] {
        assert_name_round_trip(&usage, name);
    }
    for (stage, name) in [
        (CourseStage::Placement, "Placement"),
        (CourseStage::Ready, "Ready"),
    ] {
        assert_name_round_trip(&stage, name);
    }
    for (step, tag) in [
        (PlacementStep::ChooseRating, "ChooseRating"),
        (PlacementStep::PreparingQuestions, "PreparingQuestions"),
        (
            PlacementStep::Question {
                question: placement_question(),
            },
            "Question",
        ),
        (PlacementStep::BuildingIndex, "BuildingIndex"),
        (PlacementStep::Done, "Done"),
    ] {
        assert_tagged_round_trip(&step, tag);
    }
    for (kind, name) in [
        (SectionKind::BroadPass, "BroadPass"),
        (SectionKind::DeepDive, "DeepDive"),
    ] {
        assert_name_round_trip(&kind, name);
    }
    for (mark, name) in [
        (NodeMark::Unread, "Unread"),
        (NodeMark::Read, "Read"),
        (NodeMark::Branch, "Branch"),
    ] {
        assert_name_round_trip(&mark, name);
    }
    for (style, name) in [
        (VersionStyle::Original, "Original"),
        (VersionStyle::Simpler, "Simpler"),
        (VersionStyle::MoreTechnical, "MoreTechnical"),
        (VersionStyle::MoreExamples, "MoreExamples"),
    ] {
        assert_name_round_trip(&style, name);
    }
    let block_tags = ["Prose", "Visual", "Figure", "Mermaid", "Check", "Fallback"];
    for (block, tag) in blocks().iter().zip(block_tags) {
        assert_tagged_round_trip(&block.content, tag);
    }
    for (kind, name) in [
        (BlockKind::Prose, "Prose"),
        (BlockKind::Visual, "Visual"),
        (BlockKind::Figure, "Figure"),
        (BlockKind::Mermaid, "Mermaid"),
        (BlockKind::Check, "Check"),
    ] {
        assert_name_round_trip(&kind, name);
    }
    for (result, tag) in [
        (
            WikipediaResult::Found {
                title: "Mantle".to_owned(),
                extract: "A layer.".to_owned(),
                url: "https://en.wikipedia.org/wiki/Mantle".to_owned(),
            },
            "Found",
        ),
        (WikipediaResult::NoArticle, "NoArticle"),
        (
            WikipediaResult::Failed {
                error: CoreError::Offline,
            },
            "Failed",
        ),
    ] {
        assert_tagged_round_trip(&result, tag);
    }
    for (role, name) in [
        (ChatRole::Candidate, "Candidate"),
        (ChatRole::Agent, "Agent"),
    ] {
        assert_name_round_trip(&role, name);
    }
    for (state, name) in [
        (MessageState::Complete, "Complete"),
        (MessageState::Stopped, "Stopped"),
    ] {
        assert_name_round_trip(&state, name);
    }
    for (state, tag) in [
        (UsageState::NoPlanLogin, "NoPlanLogin"),
        (UsageState::AwaitingReading, "AwaitingReading"),
        (
            UsageState::Connected {
                reading: usage_reading(Some(UsageFailure::Network)),
            },
            "Connected",
        ),
        (
            UsageState::FirstReadFailed {
                reason: UsageFailure::Unexpected,
            },
            "FirstReadFailed",
        ),
    ] {
        assert_tagged_round_trip(&state, tag);
    }
    for (level, name) in [
        (UsageLevel::Normal, "Normal"),
        (UsageLevel::High, "High"),
        (UsageLevel::Critical, "Critical"),
        (UsageLevel::Limit, "Limit"),
    ] {
        assert_name_round_trip(&level, name);
    }
    for (failure, name) in [
        (UsageFailure::Network, "Network"),
        (UsageFailure::Auth, "Auth"),
        (UsageFailure::Rejected, "Rejected"),
        (UsageFailure::Timeout, "Timeout"),
        (UsageFailure::Cancelled, "Cancelled"),
        (UsageFailure::Unexpected, "Unexpected"),
    ] {
        assert_name_round_trip(&failure, name);
    }
    for (level, name) in [
        (ContentsLevel::Chapter, "Chapter"),
        (ContentsLevel::Section, "Section"),
        (ContentsLevel::DeepDive, "DeepDive"),
    ] {
        assert_name_round_trip(&level, name);
    }
    for (result, tag) in [
        (
            ExportResult::Saved {
                path: "/tmp/a.pdf".to_owned(),
            },
            "Saved",
        ),
        (ExportResult::Cancelled, "Cancelled"),
        (
            ExportResult::Failed {
                reason: "the disk is full".to_owned(),
            },
            "Failed",
        ),
    ] {
        assert_tagged_round_trip(&result, tag);
    }
}

fn event_tag(event: &CoreEvent) -> &'static str {
    match event {
        CoreEvent::SessionChanged { .. } => "SessionChanged",
        CoreEvent::ModelsChanged { .. } => "ModelsChanged",
        CoreEvent::CoursesChanged { .. } => "CoursesChanged",
        CoreEvent::IndexChanged { .. } => "IndexChanged",
        CoreEvent::PlacementProgress { .. } => "PlacementProgress",
        CoreEvent::Generation { .. } => "Generation",
        CoreEvent::GlossaryChanged { .. } => "GlossaryChanged",
        CoreEvent::SideChatsChanged { .. } => "SideChatsChanged",
        CoreEvent::FollowUpPrompts { .. } => "FollowUpPrompts",
        CoreEvent::UsageChanged { .. } => "UsageChanged",
    }
}

fn generation_tag(event: &GenerationEvent) -> &'static str {
    match event {
        GenerationEvent::Started => "Started",
        GenerationEvent::Activity { .. } => "Activity",
        GenerationEvent::BlockOpened { .. } => "BlockOpened",
        GenerationEvent::TextDelta { .. } => "TextDelta",
        GenerationEvent::BlockCompleted { .. } => "BlockCompleted",
        GenerationEvent::CitationAdded { .. } => "CitationAdded",
        GenerationEvent::Finished { .. } => "Finished",
    }
}

#[test]
fn every_event_round_trips_with_its_stable_tag() {
    let events = vec![
        CoreEvent::SessionChanged {
            session: Session::SigningIn,
        },
        CoreEvent::ModelsChanged {
            list: ModelList {
                models: Vec::new(),
                current: None,
            },
        },
        CoreEvent::CoursesChanged {
            courses: vec![course_summary()],
        },
        CoreEvent::IndexChanged {
            course_id: id(1),
            chapters: chapters(),
        },
        CoreEvent::PlacementProgress {
            course_id: id(1),
            placement: PlacementState {
                error: Some(CoreError::Offline),
                ..placement(PlacementStep::BuildingIndex)
            },
        },
        CoreEvent::Generation {
            generation_id: id(9),
            target: GenerationTarget::Section {
                course_id: id(1),
                section_id: id(0x101),
                style: VersionStyle::Simpler,
            },
            event: GenerationEvent::Started,
        },
        CoreEvent::GlossaryChanged {
            course_id: id(1),
            entry: glossary_entry(),
        },
        CoreEvent::SideChatsChanged {
            course_id: id(1),
            chat: chat_summary(),
        },
        CoreEvent::FollowUpPrompts {
            chat_id: id(0x80),
            prompts: vec!["How fast?".to_owned(), "Where?".to_owned()],
        },
        CoreEvent::UsageChanged {
            usage: UsageState::NoPlanLogin,
        },
    ];
    let tags: Vec<_> = events.iter().map(event_tag).collect();
    assert_eq!(
        tags,
        [
            "SessionChanged",
            "ModelsChanged",
            "CoursesChanged",
            "IndexChanged",
            "PlacementProgress",
            "Generation",
            "GlossaryChanged",
            "SideChatsChanged",
            "FollowUpPrompts",
            "UsageChanged",
        ]
    );
    for event in &events {
        assert_tagged_round_trip(event, event_tag(event));
    }
}

#[test]
fn one_generation_stream_serves_sections_define_and_side_chats() {
    let targets = [
        (
            GenerationTarget::Section {
                course_id: id(1),
                section_id: id(0x101),
                style: VersionStyle::Original,
            },
            "Section",
        ),
        (
            GenerationTarget::Define {
                course_id: id(1),
                anchor: anchor(),
            },
            "Define",
        ),
        (
            GenerationTarget::SideChat {
                course_id: id(1),
                chat_id: id(0x80),
            },
            "SideChat",
        ),
    ];
    let stream = vec![
        GenerationEvent::Started,
        GenerationEvent::Activity {
            activity: Activity::Writing,
        },
        GenerationEvent::Activity {
            activity: Activity::Searching,
        },
        GenerationEvent::Activity {
            activity: Activity::Retrying { attempt: 2 },
        },
        GenerationEvent::BlockOpened {
            block: 1,
            kind: BlockKind::Visual,
        },
        GenerationEvent::TextDelta {
            block: 0,
            text: "The mantle ".to_owned(),
        },
        GenerationEvent::BlockCompleted {
            block: blocks()[1].clone(),
        },
        GenerationEvent::CitationAdded {
            citation: citation(),
            source: source(),
        },
        GenerationEvent::Finished {
            outcome: GenerationOutcome::Completed,
        },
    ];
    let tags: Vec<_> = stream.iter().map(generation_tag).collect();
    assert_eq!(
        tags,
        [
            "Started",
            "Activity",
            "Activity",
            "Activity",
            "BlockOpened",
            "TextDelta",
            "BlockCompleted",
            "CitationAdded",
            "Finished",
        ]
    );
    for (target, target_tag) in &targets {
        assert_tagged_round_trip(target, target_tag);
        for event in &stream {
            assert_tagged_round_trip(event, generation_tag(event));
            assert_tagged_round_trip(
                &CoreEvent::Generation {
                    generation_id: id(9),
                    target: target.clone(),
                    event: event.clone(),
                },
                "Generation",
            );
        }
    }
    for (activity, tag) in [
        (Activity::Writing, "Writing"),
        (Activity::Searching, "Searching"),
        (Activity::Retrying { attempt: 3 }, "Retrying"),
    ] {
        assert_tagged_round_trip(&activity, tag);
    }
    for (outcome, tag) in [
        (GenerationOutcome::Completed, "Completed"),
        (GenerationOutcome::Stopped, "Stopped"),
        (
            GenerationOutcome::Failed {
                error: CoreError::PlanLimit { resets_at: None },
            },
            "Failed",
        ),
        (
            GenerationOutcome::Incomplete {
                reason: Some("max_output_tokens".to_owned()),
            },
            "Incomplete",
        ),
    ] {
        assert_tagged_round_trip(&outcome, tag);
    }
}

fn error_tag(error: &CoreError) -> &'static str {
    match error {
        CoreError::NotFound { .. } => "NotFound",
        CoreError::InvalidRequest { .. } => "InvalidRequest",
        CoreError::PlanLimit { .. } => "PlanLimit",
        CoreError::PlanUsageOff => "PlanUsageOff",
        CoreError::PlanNotAllowed => "PlanNotAllowed",
        CoreError::SignInRequired => "SignInRequired",
        CoreError::Offline => "Offline",
        CoreError::FailedAfterRetries { .. } => "FailedAfterRetries",
        CoreError::Provider { .. } => "Provider",
        CoreError::Store { .. } => "Store",
        CoreError::Unavailable { .. } => "Unavailable",
    }
}

#[test]
fn every_error_round_trips_with_its_stable_tag() {
    let errors = vec![
        CoreError::NotFound {
            what: "course 1".to_owned(),
        },
        CoreError::InvalidRequest {
            message: "the topic is empty".to_owned(),
        },
        CoreError::PlanLimit {
            resets_at: Some(time()),
        },
        CoreError::PlanUsageOff,
        CoreError::PlanNotAllowed,
        CoreError::SignInRequired,
        CoreError::Offline,
        CoreError::FailedAfterRetries { attempts: 3 },
        CoreError::Provider {
            message: "the stream broke".to_owned(),
        },
        CoreError::Store {
            message: "disk I/O error".to_owned(),
        },
        CoreError::Unavailable {
            feature: "usage".to_owned(),
        },
    ];
    let tags: Vec<_> = errors.iter().map(error_tag).collect();
    assert_eq!(
        tags,
        [
            "NotFound",
            "InvalidRequest",
            "PlanLimit",
            "PlanUsageOff",
            "PlanNotAllowed",
            "SignInRequired",
            "Offline",
            "FailedAfterRetries",
            "Provider",
            "Store",
            "Unavailable",
        ]
    );
    for error in &errors {
        assert_tagged_round_trip(error, error_tag(error));
        assert!(!error.to_string().is_empty());
    }
    assert_tagged_round_trip(&CoreError::PlanLimit { resets_at: None }, "PlanLimit");
}

#[test]
fn a_placement_question_is_shown_without_its_answer_key() {
    let json = serde_json::to_value(placement_question()).unwrap();
    let fields: BTreeSet<_> = json.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        fields,
        ["id", "number", "options", "stem"]
            .map(str::to_owned)
            .into()
    );
}

// -- features not built yet ----------------------------------------------------

fn start(dir: &tempfile::TempDir) -> (Core, Arc<FakeProvider>, Arc<RecordingEvents>) {
    let provider = Arc::new(FakeProvider::new(FakeScript::sample()));
    let events = Arc::new(RecordingEvents::default());
    let deps: CoreDeps = FakePorts::default().deps(
        dir.path().join("atlas.sqlite3"),
        provider.clone(),
        events.clone(),
    );
    (Core::new(deps).expect("core starts"), provider, events)
}

#[tokio::test]
async fn requests_for_features_not_built_yet_answer_unavailable_naming_the_feature() {
    let dir = tempfile::tempdir().unwrap();
    let (core, provider, events) = start(&dir);
    let built = [
        "ListModels",
        "SelectModel",
        "GetSession",
        "SignIn",
        "CancelSignIn",
        "EnablePlanUsage",
        "SignOut",
        "OpenManageUsage",
        "OpenExternal",
        "ListCourses",
        "CreateCourse",
        "SubmitSelfRating",
        "AnswerPlacement",
        "RetryPlacementStep",
        "OpenCourse",
        "DeleteCourse",
        "OpenSection",
        "StopGeneration",
        "RetrySection",
        "ExplainDifferently",
        "SelectVersion",
        "AnswerCheck",
        "Next",
        "GoDeeper",
        "Define",
        "LookupWikipedia",
        "ListGlossary",
        "ListSideChats",
        "StartSideChat",
        "OpenSideChat",
        "SendSideChatMessage",
        "StopSideChatReply",
        "DeleteGlossaryEntry",
        "DeleteSideChat",
        "GetUsage",
        "SetWindowFocused",
        "GetExportDocument",
        "ExportPdf",
    ];

    let mut unbuilt = 0;
    for (request, feature) in every_request() {
        if built.contains(&request_tag(&request)) {
            continue;
        }
        unbuilt += 1;
        assert_eq!(
            core.handle(request.clone()).await,
            Err(CoreError::Unavailable {
                feature: feature.to_owned(),
            }),
            "{request:?}"
        );
    }

    assert_eq!(unbuilt, 0);
    assert!(provider.requests().is_empty());
    assert_eq!(events.take(), []);
    // The thin loop still answers beside them.
    assert_eq!(
        core.handle(CoreRequest::ListCourses).await,
        Ok(CoreResponse::Courses {
            courses: Vec::new()
        })
    );
}

// -- generated bindings ----------------------------------------------------------

/// Every exported type reachable from the four protocol roots, by file name,
/// with the TypeScript ts-rs generates for it now.
fn reachable_bindings() -> BTreeMap<String, String> {
    struct Reach {
        config: Config,
        seen: HashSet<TypeId>,
        bindings: BTreeMap<String, String>,
    }
    impl TypeVisitor for Reach {
        fn visit<T: TS + 'static + ?Sized>(&mut self) {
            if !self.seen.insert(TypeId::of::<T>()) {
                return;
            }
            if let Some(path) = T::output_path() {
                let text = T::export_to_string(&self.config)
                    .unwrap_or_else(|error| panic!("{path:?} exports: {error}"));
                self.bindings
                    .insert(path.to_string_lossy().into_owned(), text);
            }
            T::visit_dependencies(self);
        }
    }
    let mut reach = Reach {
        config: Config::from_env(),
        seen: HashSet::new(),
        bindings: BTreeMap::new(),
    };
    reach.visit::<CoreRequest>();
    reach.visit::<CoreResponse>();
    reach.visit::<CoreEvent>();
    reach.visit::<CoreError>();
    reach.bindings
}

fn generated_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/core/generated")
}

fn generated_files() -> BTreeMap<String, String> {
    std::fs::read_dir(generated_dir())
        .expect("the generated bindings are committed")
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_committed_bindings_are_exactly_what_the_protocol_generates() {
    let reachable = reachable_bindings();
    let generated = generated_files();
    assert_eq!(
        generated.keys().collect::<Vec<_>>(),
        reachable.keys().collect::<Vec<_>>(),
        "regenerate with `npm --prefix ui run gen:types`"
    );
    for (file, text) in &reachable {
        assert_eq!(&generated[file], text, "{file} is stale");
    }
}

#[test]
fn the_generated_requests_have_no_usage_login_and_the_two_deletes() {
    let request = &generated_files()["CoreRequest.ts"];
    for removed in ["\"ConnectUsage\"", "\"CancelConnectUsage\""] {
        assert!(!request.contains(removed), "{removed} is still generated");
    }
    for added in [
        "\"type\": \"DeleteGlossaryEntry\", entryId: string",
        "\"type\": \"DeleteSideChat\", chatId: string",
    ] {
        assert!(request.contains(added), "{added} is not generated");
    }
}

#[test]
fn no_core_only_or_token_bearing_type_reaches_the_interface() {
    let reachable = reachable_bindings();
    let core_only = [
        "Ladder",
        "Rung",
        "RungId",
        "CandidateProfile",
        "RungKnowledge",
        "Knowledge",
        "Misconception",
        "CourseMap",
        "Concept",
        "MapEntry",
        "Convention",
        "ConventionKind",
        "ConventionOrigin",
        "SectionRecord",
        "StoredRecord",
        "RecordStatus",
        "TaughtConcept",
        "FixedEntry",
        "Promise",
        "KeyedPlacementQuestion",
        "KeyedCheckQuestion",
        "KeyedOption",
        "AccessToken",
        "RefreshToken",
    ];
    for name in core_only {
        assert!(
            !reachable.contains_key(&format!("{name}.ts")),
            "{name} is reachable from the protocol"
        );
    }
    // Neither the types nor their fields appear in any generated text.
    let words = [
        "token",
        "ladder",
        "rung",
        "concept",
        "convention",
        "misconception",
        "profile",
        "sectionrecord",
        "coursemap",
        "promise",
    ];
    for (file, text) in reachable.iter().chain(&generated_files()) {
        let text = text.to_lowercase();
        for word in words {
            assert!(!text.contains(word), "{file} mentions {word:?}");
        }
    }
}

// -- the sample course ---------------------------------------------------------

fn assert_json_round_trip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let json = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, value, "{json}");
}

#[test]
fn the_sample_course_memory_round_trips_through_json() {
    let course = sample_course();
    assert_json_round_trip(&course.ladder);
    assert_json_round_trip(&course.profile);
    assert_json_round_trip(&course.map);
    assert_json_round_trip(&course.conventions);
    assert!(!course.records.is_empty());
    for sample in &course.records {
        assert_json_round_trip(&sample.record);
        assert_json_round_trip(&sample.record.record);
    }
}

#[test]
fn a_section_record_reads_the_json_the_model_writes() {
    let written = json!({
        "summary": "Plates are rigid pieces of the lithosphere.",
        "concepts": [{"name": "tectonic plate", "line": "A rigid piece that moves as one."}],
        "notation": [{"key": "v", "meaning": "plate velocity, in cm per year"}],
        "terms": [{"key": "plate", "meaning": "a rigid piece of the lithosphere"}],
        "examples": ["the Atlantic widening"],
        "promises": [{"section": "2.2", "what": "what makes the plates move"}],
    });

    let record: SectionRecord = serde_json::from_value(written.clone()).unwrap();

    assert_eq!(record.concepts[0].name, "tectonic plate");
    assert_eq!(record.promises[0].section, "2.2");
    assert_eq!(serde_json::to_value(&record).unwrap(), written);
}

#[test]
fn the_sample_map_is_valid_and_every_read_section_has_one_record() {
    let course = sample_course();
    // Index order: depth first, deep dives directly under their section.
    fn walk(sections: &[atlas_core::index_tree::TreeSection], out: &mut Vec<(Uuid, bool)>) {
        for section in sections {
            out.push((section.id, section.read));
            walk(&section.children, out);
        }
    }
    let mut order = Vec::new();
    for chapter in &course.index.chapters {
        walk(&chapter.sections, &mut order);
    }
    assert_eq!(course.index.chapters.len(), 3);
    assert_eq!(order.len(), 9);

    let rungs: HashSet<_> = course.ladder.rungs.iter().map(|rung| rung.id).collect();
    for concept in &course.map.concepts {
        assert!(!concept.name.is_empty());
        if let Some(rung) = concept.rung {
            assert!(rungs.contains(&rung), "{concept:?}");
        }
    }
    let mut introduced = HashSet::new();
    for (section, _) in &order {
        let entry = course
            .map
            .entries
            .iter()
            .find(|entry| entry.section == *section)
            .unwrap_or_else(|| panic!("no map entry for {section}"));
        for concept in &entry.relies_on {
            assert!(
                introduced.contains(concept),
                "{section} relies on {concept}, not introduced earlier"
            );
        }
        introduced.extend(entry.introduces.iter().cloned());
    }

    let read: Vec<_> = order
        .iter()
        .filter(|(_, read)| *read)
        .map(|(section, _)| *section)
        .collect();
    let recorded: Vec<_> = course.records.iter().map(|sample| sample.section).collect();
    assert_eq!(recorded, read);
    assert_eq!(
        read,
        [
            sample_course::LAYERS,
            sample_course::LITHOSPHERE,
            sample_course::WHAT_A_PLATE_IS,
            sample_course::HOW_PLATES_ARE_MAPPED,
        ]
    );
}

// -- the test server -------------------------------------------------------------

#[tokio::test]
async fn the_test_server_answers_scripted_routes_and_records_requests() {
    let server = TestServer::start([
        Route::get("/api/models", [Reply::json(200, &json!({"models": []}))]),
        Route::post(
            "/api/responses",
            [
                Reply::json(503, &json!({"detail": "busy"})),
                Reply::sse("data: done\n\n").header("x-request-id", "req_1"),
            ],
        ),
    ])
    .await;
    assert_eq!(server.url().host_str(), Some("127.0.0.1"));
    assert_ne!(server.url().port(), Some(0));
    let client = reqwest::Client::new();
    let url = |path: &str| server.url().join(path).unwrap();

    let models = client.get(url("api/models?all=1")).send().await.unwrap();
    assert_eq!(models.status(), 200);
    assert_eq!(models.text().await.unwrap(), r#"{"models":[]}"#);

    let post = |body: &'static str| {
        client
            .post(url("api/responses"))
            .header("authorization", "Bearer test")
            .body(body)
            .send()
    };
    assert_eq!(post("first").await.unwrap().status(), 503);
    let second = post("second").await.unwrap();
    assert_eq!(second.status(), 200);
    assert_eq!(second.headers()["x-request-id"], "req_1");
    assert_eq!(second.text().await.unwrap(), "data: done\n\n");
    // The last reply repeats.
    assert_eq!(post("third").await.unwrap().status(), 200);
    assert_eq!(
        client.get(url("missing")).send().await.unwrap().status(),
        404
    );

    let requests = server.requests();
    let seen: Vec<_> = requests
        .iter()
        .map(|request| (request.method.as_str(), request.path.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            ("GET", "/api/models"),
            ("POST", "/api/responses"),
            ("POST", "/api/responses"),
            ("POST", "/api/responses"),
            ("GET", "/missing"),
        ]
    );
    assert_eq!(requests[0].query.as_deref(), Some("all=1"));
    assert_eq!(requests[1].header("Authorization"), Some("Bearer test"));
    assert_eq!(requests[2].body_text(), "second");
}

#[test]
fn local_endpoints_put_every_service_under_one_server() {
    let base = url::Url::parse("http://127.0.0.1:4321/").unwrap();
    let endpoints = atlas_core::ports::Endpoints::local(&base);
    let all = [
        &endpoints.api,
        &endpoints.auth,
        &endpoints.chatgpt,
        &endpoints.wikipedia,
    ];
    let paths: HashSet<_> = all.iter().map(|url| url.path()).collect();
    assert_eq!(paths.len(), 4, "each service has its own path");
    for url in all {
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(4321));
    }
    let production = atlas_core::ports::Endpoints::production();
    assert_eq!(
        production.api.as_str(),
        "https://chatgpt.com/backend-api/codex"
    );
    assert_eq!(production.chatgpt.host_str(), Some("chatgpt.com"));
}
