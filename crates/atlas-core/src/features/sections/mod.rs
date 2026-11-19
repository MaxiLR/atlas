//! Opening a section: its current version from the store, or a generation
//! streamed as events and stored once complete.
//!
//! For now the thin loop: one prose block written from the index alone.

use std::fmt::Write as _;
use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::harness::prompts;
use crate::ports::EventSink;
use crate::protocol::{
    Block, BlockContent, BlockKind, CoreError, CoreEvent, GenerationEvent, GenerationOutcome,
    GenerationTarget, SectionVersion, SectionView, VersionStyle, VersionSummary,
};
use crate::provider::{ModelProvider, ProviderRequest, Task, complete_text};
use crate::store::{Store, StoredSection, StoredVersion};

const FEATURE: &str = "sections";

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
    events: Arc<dyn EventSink>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
            events: deps.events.clone(),
        })
    }

    /// Answers from the store, or generates the section, streaming it as
    /// events, and stores it once the provider reports completion.
    pub(crate) async fn open(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        let section = self.section(course_id, section_id)?;
        if let Some(version) = &section.latest {
            return Ok(view(&section, version));
        }
        cx.session.require_plan(cx).await?;
        self.generate(cx, &section).await?;
        let section = self.section(course_id, section_id)?;
        let version = section.latest.as_ref().ok_or_else(|| CoreError::Store {
            message: format!("section {section_id} was not stored"),
        })?;
        Ok(view(&section, version))
    }

    pub(crate) async fn stop(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _section_id: Uuid,
    ) -> Result<(), CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn retry(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _section_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        Err(unavailable(FEATURE))
    }

    fn section(&self, course_id: Uuid, section_id: Uuid) -> Result<StoredSection, CoreError> {
        self.store
            .section(course_id, section_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("section {section_id}"),
            })
    }

    /// Writes the section as one prose block and stores it; the events end
    /// with the outcome either way.
    async fn generate(&self, cx: &Features, section: &StoredSection) -> Result<(), CoreError> {
        let course = cx.library.course(section.course_id)?;
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

        let generation_id = Uuid::new_v4();
        let target = GenerationTarget::Section {
            course_id: section.course_id,
            section_id: section.id,
            style: VersionStyle::Original,
        };
        let emit = |event| {
            self.events.emit(CoreEvent::Generation {
                generation_id,
                target: target.clone(),
                event,
            });
        };
        emit(GenerationEvent::Started);
        emit(GenerationEvent::BlockOpened {
            block: 0,
            kind: BlockKind::Prose,
        });
        let written = async {
            let text = complete_text(
                self.provider.as_ref(),
                ProviderRequest::new(Task::WriteSection, prompts::WRITE_SECTION.text(), input),
                |text| {
                    emit(GenerationEvent::TextDelta {
                        block: 0,
                        text: text.to_owned(),
                    });
                },
            )
            .await?;
            emit(GenerationEvent::BlockCompleted {
                block: Block {
                    index: 0,
                    content: BlockContent::Prose { text: text.clone() },
                },
            });
            self.store
                .insert_section_version(section.id, &text, &prompts::WRITE_SECTION.id())?;
            Ok::<_, CoreError>(())
        }
        .await;
        emit(GenerationEvent::Finished {
            outcome: match &written {
                Ok(()) => GenerationOutcome::Completed,
                Err(error) => GenerationOutcome::Failed {
                    error: error.clone(),
                },
            },
        });
        written
    }
}

fn view(section: &StoredSection, version: &StoredVersion) -> SectionView {
    SectionView {
        id: section.id,
        course_id: section.course_id,
        title: section.title.clone(),
        summary: section.summary.clone(),
        version: SectionVersion {
            id: version.id,
            style: VersionStyle::Original,
            blocks: vec![Block {
                index: 0,
                content: BlockContent::Prose {
                    text: version.content.clone(),
                },
            }],
            citations: Vec::new(),
            sources: Vec::new(),
            written_without_search: false,
            model: None,
            created_at: version.created_at,
        },
        versions: vec![VersionSummary {
            id: version.id,
            style: VersionStyle::Original,
            created_at: version.created_at,
        }],
    }
}
