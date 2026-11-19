use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use uuid::Uuid;

use crate::harness::index::IndexDraft;
use crate::harness::prompts;
use crate::ports::EventSink;
use crate::protocol::{CoreError, CoreEvent, CoreRequest, CoreResponse, CourseView, SectionView};
use crate::provider::{ModelProvider, ProviderError, ProviderEvent, ProviderRequest, Task};
use crate::store::{Store, StoreError};

/// Everything the core is built from.
pub struct CoreDeps {
    /// The SQLite database file; created if missing.
    pub database_path: PathBuf,
    pub provider: Arc<dyn ModelProvider>,
    pub events: Arc<dyn EventSink>,
}

/// The core's single entry point: one typed request in, one typed response
/// out, events on the side.
pub struct Core {
    store: Store,
    provider: Arc<dyn ModelProvider>,
    events: Arc<dyn EventSink>,
}

impl From<StoreError> for CoreError {
    fn from(error: StoreError) -> Self {
        CoreError::Store {
            message: error.to_string(),
        }
    }
}

impl From<ProviderError> for CoreError {
    fn from(error: ProviderError) -> Self {
        CoreError::Provider {
            message: error.to_string(),
        }
    }
}

impl Core {
    pub fn new(deps: CoreDeps) -> Result<Self, CoreError> {
        Ok(Self {
            store: Store::open(&deps.database_path)?,
            provider: deps.provider,
            events: deps.events,
        })
    }

    pub async fn handle(&self, request: CoreRequest) -> Result<CoreResponse, CoreError> {
        match request {
            CoreRequest::ListCourses => Ok(CoreResponse::Courses {
                courses: self.store.list_courses()?,
            }),
            CoreRequest::CreateCourse { topic } => Ok(CoreResponse::Course {
                course: self.create_course(&topic).await?,
            }),
            CoreRequest::OpenCourse { course_id } => Ok(CoreResponse::Course {
                course: self.course(course_id)?,
            }),
            CoreRequest::OpenSection {
                course_id,
                section_id,
            } => Ok(CoreResponse::Section {
                section: self.open_section(course_id, section_id).await?,
            }),
            CoreRequest::Next {
                course_id,
                section_id,
            } => self.next(course_id, section_id).await,
        }
    }

    fn course(&self, course_id: Uuid) -> Result<CourseView, CoreError> {
        self.store
            .course(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })
    }

    async fn create_course(&self, topic: &str) -> Result<CourseView, CoreError> {
        let topic = topic.trim();
        if topic.is_empty() {
            return Err(CoreError::InvalidRequest {
                message: "the topic is empty".to_owned(),
            });
        }
        let text = self
            .complete(
                ProviderRequest {
                    task: Task::BuildIndex,
                    instructions: prompts::BUILD_INDEX.text().to_owned(),
                    input: format!("Topic: {topic}"),
                },
                |_| {},
            )
            .await?;
        let index: IndexDraft =
            serde_json::from_str(&text).map_err(|error| CoreError::Provider {
                message: format!("the index is not valid: {error}"),
            })?;
        if index.chapters.len() < 2 || index.chapters.iter().any(|c| c.sections.is_empty()) {
            return Err(CoreError::Provider {
                message: "the index needs at least two chapters, each with sections".to_owned(),
            });
        }
        Ok(self.store.insert_course(topic, &index)?)
    }

    /// Answers from the store, or generates the section, streaming its text as
    /// events, and stores it once the provider reports completion.
    async fn open_section(
        &self,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        let section =
            self.store
                .section(course_id, section_id)?
                .ok_or_else(|| CoreError::NotFound {
                    what: format!("section {section_id}"),
                })?;
        let view = |content: String| SectionView {
            id: section.id,
            course_id,
            title: section.title.clone(),
            content,
        };
        if let Some(content) = &section.content {
            return Ok(view(content.clone()));
        }

        let course = self.course(course_id)?;
        let mut input = format!("Topic: {}\n\nIndex:\n", course.topic);
        for chapter in &course.chapters {
            let _ = writeln!(input, "- {}", chapter.title);
            for entry in &chapter.sections {
                let _ = writeln!(input, "  - {}: {}", entry.title, entry.summary);
            }
        }
        let _ = write!(
            input,
            "\nSection to write: {}: {}",
            section.title, section.summary
        );
        let content = self
            .complete(
                ProviderRequest {
                    task: Task::WriteSection,
                    instructions: prompts::WRITE_SECTION.text().to_owned(),
                    input,
                },
                |text| {
                    self.events.emit(CoreEvent::SectionDelta {
                        course_id,
                        section_id,
                        text: text.to_owned(),
                    })
                },
            )
            .await?;
        self.store
            .insert_section_version(section_id, &content, &prompts::WRITE_SECTION.id())?;
        self.events.emit(CoreEvent::SectionCompleted {
            course_id,
            section_id,
        });
        Ok(view(content))
    }

    async fn next(&self, course_id: Uuid, section_id: Uuid) -> Result<CoreResponse, CoreError> {
        let course = self.course(course_id)?;
        let mut sections = course
            .chapters
            .iter()
            .flat_map(|chapter| &chapter.sections)
            .skip_while(|section| section.id != section_id);
        if sections.next().is_none() {
            return Err(CoreError::NotFound {
                what: format!("section {section_id}"),
            });
        }
        match sections.next() {
            Some(following) => Ok(CoreResponse::Section {
                section: self.open_section(course_id, following.id).await?,
            }),
            None => Ok(CoreResponse::EndOfCourse { course_id }),
        }
    }

    /// Runs one provider request to completion, passing each text delta to
    /// `on_delta`. Only a stream that reports completion counts.
    async fn complete(
        &self,
        request: ProviderRequest,
        mut on_delta: impl FnMut(&str),
    ) -> Result<String, CoreError> {
        let mut stream = self.provider.stream(request).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event? {
                ProviderEvent::TextDelta(delta) => {
                    on_delta(&delta);
                    text.push_str(&delta);
                }
                ProviderEvent::Completed => return Ok(text),
            }
        }
        Err(ProviderError::Incomplete.into())
    }
}
