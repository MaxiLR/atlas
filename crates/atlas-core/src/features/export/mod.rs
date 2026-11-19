//! PDF export: the course as it stands, read from the store only, printed
//! through the `PdfExporter` port.
//!
//! A section is included once it has a stored version, in its current one;
//! every other entry is listed in the contents by its title only. A chapter
//! is included when any section under it is. Side chats are never read.

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::index_tree::{self, IndexEntry};
use crate::ports::{PdfExporter, PdfOutcome};
use crate::protocol::{
    ContentsEntry, ContentsLevel, CoreError, ExportDocument, ExportResult, ExportSection,
    SectionKind, SectionVersion,
};
use crate::store::{CourseRecord, Store, StoredSectionVersion};

pub(crate) struct Feature {
    store: Arc<Store>,
    pdf: Arc<dyn PdfExporter>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            pdf: deps.pdf.clone(),
        })
    }

    /// The course's title, its contents, every section with a stored version
    /// in its current version, and the glossary, all in index order.
    pub(crate) async fn document(
        &self,
        _cx: &Features,
        course_id: Uuid,
    ) -> Result<ExportDocument, CoreError> {
        let course = self.record(course_id)?;
        let tree = self.store.index_tree(course_id)?;
        let entries = index_tree::in_index_order(&tree);
        let mut sections = Vec::new();
        for entry in &entries {
            if let Some(version) = self.store.current_version(entry.section.id)? {
                sections.push(export_section(entry, version));
            }
        }
        let included = |id: Uuid| sections.iter().any(|section| section.node_id == id);
        let mut contents = Vec::with_capacity(tree.chapters.len() + entries.len());
        for chapter in &tree.chapters {
            let in_chapter = entries.iter().filter(|entry| entry.chapter == chapter.id);
            contents.push(ContentsEntry {
                node_id: chapter.id,
                level: ContentsLevel::Chapter,
                depth: 0,
                title: chapter.title.clone(),
                included: in_chapter.clone().any(|entry| included(entry.section.id)),
            });
            contents.extend(in_chapter.map(|entry| ContentsEntry {
                node_id: entry.section.id,
                level: match entry.kind {
                    SectionKind::BroadPass => ContentsLevel::Section,
                    SectionKind::DeepDive => ContentsLevel::DeepDive,
                },
                depth: entry.depth + 1,
                title: entry.section.title.clone(),
                included: included(entry.section.id),
            }));
        }
        Ok(ExportDocument {
            course_id,
            title: course.topic,
            contents,
            sections,
            glossary: self.store.glossary(course_id)?,
        })
    }

    /// Asks the exporter to print the course to a file the candidate picks.
    pub(crate) async fn export_pdf(
        &self,
        _cx: &Features,
        course_id: Uuid,
    ) -> Result<ExportResult, CoreError> {
        let course = self.record(course_id)?;
        let suggested_name = format!("{}.pdf", course.topic);
        Ok(match self.pdf.export(course_id, &suggested_name).await {
            PdfOutcome::Saved(path) => ExportResult::Saved {
                path: path.display().to_string(),
            },
            PdfOutcome::Cancelled => ExportResult::Cancelled,
            PdfOutcome::Failed(reason) => ExportResult::Failed { reason },
        })
    }

    fn record(&self, course_id: Uuid) -> Result<CourseRecord, CoreError> {
        self.store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })
    }
}

fn export_section(entry: &IndexEntry<'_>, version: StoredSectionVersion) -> ExportSection {
    ExportSection {
        node_id: entry.section.id,
        kind: entry.kind,
        depth: entry.depth + 1,
        title: entry.section.title.clone(),
        summary: entry.section.line.clone(),
        version: SectionVersion {
            id: version.id,
            style: version.style,
            blocks: version.blocks,
            citations: version.citations,
            sources: version.sources,
            written_without_search: version.written_without_search,
            model: version.model,
            created_at: version.created_at,
        },
    }
}
