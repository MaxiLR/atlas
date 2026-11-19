use std::path::PathBuf;
use std::sync::Arc;

use crate::features::Features;
use crate::ports::{Browser, Clock, Endpoints, EventSink, PdfExporter, SecretStore};
use crate::protocol::{CoreError, CoreRequest, CoreResponse};
use crate::provider::ModelProvider;
use crate::store::Store;

/// Everything the core is built from. Tests build it with
/// [`crate::testing::FakePorts`].
pub struct CoreDeps {
    /// The SQLite database file; created if missing.
    pub database_path: PathBuf,
    pub provider: Arc<dyn ModelProvider>,
    pub events: Arc<dyn EventSink>,
    pub clock: Arc<dyn Clock>,
    pub secrets: Arc<dyn SecretStore>,
    pub browser: Arc<dyn Browser>,
    pub pdf: Arc<dyn PdfExporter>,
    pub endpoints: Endpoints,
}

/// The core's single entry point: one typed request in, one typed response
/// out, events on the side.
pub struct Core {
    features: Features,
}

impl Core {
    /// Opens the store and builds every feature from the deps.
    pub fn new(deps: CoreDeps) -> Result<Self, CoreError> {
        let store = Arc::new(Store::open(&deps.database_path, deps.clock.clone())?);
        Ok(Self {
            features: Features::new(&deps, &store)?,
        })
    }

    /// Routes a request to the feature that answers it.
    pub async fn handle(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        let cx = &self.features;
        if spends_plan(&request) {
            cx.session.require_plan(cx).await?;
        }
        let done = |()| CoreResponse::Done;
        let session = |session| CoreResponse::Session { session };
        let models = |list| CoreResponse::Models { list };
        let section = |section| CoreResponse::Section { section };
        let usage = |usage| CoreResponse::Usage { usage };
        match request {
            CoreRequest::GetSession => cx.session.get(cx).await.map(session),
            CoreRequest::SignIn => cx.session.sign_in(cx).await.map(session),
            CoreRequest::CancelSignIn => cx.session.cancel_sign_in(cx).await.map(session),
            CoreRequest::EnablePlanUsage => cx.session.enable_plan_usage(cx).await.map(session),
            CoreRequest::SignOut => cx.session.sign_out(cx).await.map(session),
            CoreRequest::OpenManageUsage => cx.external.open_manage_usage(cx).await.map(done),
            CoreRequest::OpenExternal { url } => cx.external.open_external(cx, url).await.map(done),

            CoreRequest::ListModels => cx.models.list(cx).await.map(models),
            CoreRequest::SelectModel { slug } => cx.models.select(cx, slug).await.map(models),

            CoreRequest::ListCourses => cx
                .library
                .list_courses(cx)
                .await
                .map(|courses| CoreResponse::Courses { courses }),
            CoreRequest::CreateCourse { topic } => cx
                .placement
                .create_course(cx, &topic)
                .await
                .map(|course| CoreResponse::Course { course }),
            CoreRequest::OpenCourse { course_id } => cx
                .library
                .open_course(cx, course_id)
                .await
                .map(|course| CoreResponse::Course { course }),
            CoreRequest::DeleteCourse { course_id } => {
                cx.library.delete_course(cx, course_id).await.map(done)
            }

            CoreRequest::SubmitSelfRating {
                course_id,
                rating,
                goal,
            } => cx
                .placement
                .submit_self_rating(cx, course_id, rating, goal)
                .await
                .map(|placement| CoreResponse::Placement {
                    course_id,
                    placement,
                }),
            CoreRequest::AnswerPlacement {
                course_id,
                question_id,
                answer,
            } => cx
                .placement
                .answer(cx, course_id, question_id, answer)
                .await
                .map(|placement| CoreResponse::Placement {
                    course_id,
                    placement,
                }),
            CoreRequest::RetryPlacementStep { course_id } => cx
                .placement
                .retry_step(cx, course_id)
                .await
                .map(|placement| CoreResponse::Placement {
                    course_id,
                    placement,
                }),

            CoreRequest::OpenSection {
                course_id,
                section_id,
            } => cx
                .sections
                .open(cx, course_id, section_id)
                .await
                .map(section),
            CoreRequest::StopGeneration {
                course_id,
                section_id,
            } => cx.sections.stop(cx, course_id, section_id).await.map(done),
            CoreRequest::RetrySection {
                course_id,
                section_id,
            } => cx
                .sections
                .retry(cx, course_id, section_id)
                .await
                .map(section),
            CoreRequest::ExplainDifferently {
                course_id,
                section_id,
                style,
            } => cx
                .versions
                .explain_differently(cx, course_id, section_id, style)
                .await
                .map(section),
            CoreRequest::SelectVersion {
                course_id,
                section_id,
                version_id,
            } => cx
                .versions
                .select_version(cx, course_id, section_id, version_id)
                .await
                .map(section),
            CoreRequest::AnswerCheck {
                version_id,
                block,
                question,
                choice,
            } => cx
                .checks
                .answer(cx, version_id, block, question, choice)
                .await
                .map(|result| CoreResponse::CheckAnswered { result }),
            CoreRequest::Next {
                course_id,
                section_id,
            } => cx.navigation.next(cx, course_id, section_id).await,
            CoreRequest::GoDeeper {
                course_id,
                section_id,
            } => cx
                .deep_dives
                .go_deeper(cx, course_id, section_id)
                .await
                .map(|deep_dives| CoreResponse::DeepDives {
                    course_id,
                    section_id,
                    deep_dives,
                }),

            CoreRequest::Define { anchor, sentence } => cx
                .define
                .define(cx, anchor, sentence)
                .await
                .map(|entry| CoreResponse::Definition { entry }),
            CoreRequest::LookupWikipedia { entry_id } => cx
                .wikipedia
                .lookup(cx, entry_id)
                .await
                .map(|result| CoreResponse::Wikipedia { entry_id, result }),
            CoreRequest::ListGlossary { course_id } => cx
                .define
                .list_glossary(cx, course_id)
                .await
                .map(|entries| CoreResponse::Glossary { course_id, entries }),
            CoreRequest::ListSideChats { course_id } => cx
                .side_chats
                .list(cx, course_id)
                .await
                .map(|chats| CoreResponse::SideChats { course_id, chats }),
            CoreRequest::StartSideChat { course_id, anchor } => cx
                .side_chats
                .start(cx, course_id, anchor)
                .await
                .map(|chat| CoreResponse::SideChat { chat }),
            CoreRequest::OpenSideChat { chat_id } => cx
                .side_chats
                .open(cx, chat_id)
                .await
                .map(|chat| CoreResponse::SideChat { chat }),
            CoreRequest::SendSideChatMessage { chat_id, text } => cx
                .side_chats
                .send(cx, chat_id, text)
                .await
                .map(|message| CoreResponse::SideChatReply { chat_id, message }),
            CoreRequest::StopSideChatReply { chat_id } => {
                cx.side_chats.stop_reply(cx, chat_id).await.map(done)
            }

            CoreRequest::GetUsage => cx.usage.get(cx).await.map(usage),
            CoreRequest::ConnectUsage => cx.usage.connect(cx).await.map(usage),
            CoreRequest::CancelConnectUsage => cx.usage.cancel_connect(cx).await.map(usage),
            CoreRequest::SetWindowFocused { focused } => {
                cx.usage.set_window_focused(cx, focused).await.map(done)
            }

            CoreRequest::GetExportDocument { course_id } => cx
                .export
                .document(cx, course_id)
                .await
                .map(|document| CoreResponse::ExportDocument { document }),
            CoreRequest::ExportPdf { course_id } => cx
                .export
                .export_pdf(cx, course_id)
                .await
                .map(|result| CoreResponse::Exported { result }),
        }
    }
}

/// Requests that always spend plan usage. Opening a section spends it only
/// when the section is generated, so the sections feature checks for itself.
fn spends_plan(request: &CoreRequest) -> bool {
    matches!(
        request,
        CoreRequest::CreateCourse { .. }
            | CoreRequest::SubmitSelfRating { .. }
            | CoreRequest::AnswerPlacement { .. }
            | CoreRequest::RetryPlacementStep { .. }
            | CoreRequest::RetrySection { .. }
            | CoreRequest::ExplainDifferently { .. }
            | CoreRequest::GoDeeper { .. }
            | CoreRequest::Define { .. }
            | CoreRequest::SendSideChatMessage { .. }
    )
}
