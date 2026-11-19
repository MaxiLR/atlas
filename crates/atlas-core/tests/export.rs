//! The export document and the PDF export through the core's request
//! handler, on a database in a temp dir, with a provider that records every
//! request and a fake exporter.
//!
//! The course is laid down through the store's repository API on the same
//! file: the sample course's index, with read broad-pass sections, an opened
//! and two unopened deep dives, unopened sections, two versions of one
//! section, a glossary and side chats.

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::Core;
use atlas_core::ports::PdfOutcome;
use atlas_core::protocol::{
    Anchor, Block, BlockContent, ChatRole, Citation, ContentsEntry, ContentsLevel, CoreError,
    CoreRequest, CoreResponse, ExportDocument, ExportResult, ExportSection, MessageState,
    SectionKind, SectionVersion, Source, VersionStyle,
};
use atlas_core::provider::{FakeProvider, FakeScript};
use atlas_core::store::{NewChatMessage, NewGlossaryEntry, NewSectionVersion, Store};
use atlas_core::testing::{FakePdfExporter, FakePorts, RecordingEvents};
use support::sample_course::{
    CHAPTER_BOUNDARIES, CHAPTER_MOTION, CHAPTER_STRUCTURE, HOW_PLATES_ARE_MAPPED,
    HOW_PLATES_ARE_MAPPED_V1, KINDS_OF_BOUNDARY, LAYERS, LAYERS_V1, LITHOSPHERE, LITHOSPHERE_V1,
    MICROPLATES, PLATE_THICKNESS, SUBDUCTION_ZONES, TOPIC, WHAT_A_PLATE_IS, WHAT_A_PLATE_IS_V1,
    WHAT_DRIVES_PLATES, sample_course,
};
use tempfile::TempDir;
use uuid::Uuid;

/// The second, simpler version of "Lithosphere and asthenosphere".
const LITHOSPHERE_V2: Uuid = Uuid::from_u128(0x2102);

const SIDE_CHAT_QUESTION: &str = "Is the asthenosphere actually liquid?";
const SIDE_CHAT_ANSWER: &str =
    "No: it is solid rock that flows like warm wax over millions of years.";
const COURSE_CHAT_QUESTION: &str = "Which book should I read next about mountains?";

fn prose(index: u32, text: &str) -> Block {
    Block {
        index,
        content: BlockContent::Prose {
            text: text.to_owned(),
        },
    }
}

/// The text each version stored carries, so a test can tell versions apart.
fn text_of(version: Uuid) -> String {
    format!("The text of version {version}.")
}

fn layers_citations() -> Vec<Citation> {
    vec![Citation {
        block: 0,
        start: 0,
        end: 9,
        source: 1,
    }]
}

fn layers_sources() -> Vec<Source> {
    vec![Source {
        number: 1,
        url: "https://www.usgs.gov/earth-layers".to_owned(),
        title: "The Earth's layers".to_owned(),
    }]
}

struct Harness {
    _dir: TempDir,
    core: Core,
    provider: Arc<FakeProvider>,
    pdf: Arc<FakePdfExporter>,
    /// The same database file, for laying the course down.
    store: Store,
    course: Uuid,
}

impl Harness {
    fn new() -> Self {
        Self::exporting(PdfOutcome::Cancelled)
    }

    /// A course laid down, with the exporter answering `outcome`.
    fn exporting(outcome: PdfOutcome) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("atlas.sqlite3");
        let pdf = Arc::new(FakePdfExporter::answering(outcome));
        let ports = FakePorts {
            pdf: pdf.clone(),
            ..FakePorts::default()
        };
        let provider = Arc::new(FakeProvider::new(FakeScript::sample()));
        let events = Arc::new(RecordingEvents::default());
        let core =
            Core::new(ports.deps(path.clone(), provider.clone(), events)).expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();

        let sample = sample_course();
        let course = store.create_course(&sample.topic).unwrap().id;
        store
            .insert_index(course, &sample.index, &sample.map, &[])
            .unwrap();
        let harness = Self {
            _dir: dir,
            core,
            provider,
            pdf,
            store,
            course,
        };
        harness.lay_down();
        harness
    }

    fn insert(&self, id: Uuid, node: Uuid, style: VersionStyle, citations: bool) {
        let record = sample_course()
            .records
            .into_iter()
            .find(|record| record.section == node)
            .expect("a sample record for the section")
            .record;
        self.store
            .insert_version(
                &NewSectionVersion {
                    id,
                    node,
                    style,
                    blocks: vec![prose(0, &text_of(id))],
                    checks: Vec::new(),
                    citations: if citations {
                        layers_citations()
                    } else {
                        Vec::new()
                    },
                    sources: if citations {
                        layers_sources()
                    } else {
                        Vec::new()
                    },
                    written_without_search: false,
                    record,
                    model: Some("gpt-5".to_owned()),
                    prompt: "write-section@1".to_owned(),
                },
                &[],
            )
            .unwrap();
    }

    /// Versions for the read sections (two for the lithosphere, the second
    /// current), a glossary, and side chats on a version and on the course.
    fn lay_down(&self) {
        self.insert(LAYERS_V1, LAYERS, VersionStyle::Original, true);
        self.insert(LITHOSPHERE_V1, LITHOSPHERE, VersionStyle::Original, false);
        self.insert(
            WHAT_A_PLATE_IS_V1,
            WHAT_A_PLATE_IS,
            VersionStyle::Original,
            false,
        );
        self.insert(
            HOW_PLATES_ARE_MAPPED_V1,
            HOW_PLATES_ARE_MAPPED,
            VersionStyle::Original,
            false,
        );
        self.insert(LITHOSPHERE_V2, LITHOSPHERE, VersionStyle::Simpler, false);

        self.define(LAYERS_V1, "crust", "The thin rocky skin of the Earth.");
        self.define(
            WHAT_A_PLATE_IS_V1,
            "tectonic plate",
            "A rigid piece of the lithosphere that moves as one.",
        );

        let anchored = self
            .store
            .start_side_chat(
                self.course,
                Some(&Anchor {
                    version_id: LITHOSPHERE_V2,
                    block: 0,
                    quote: "version".to_owned(),
                }),
            )
            .unwrap();
        self.chat(anchored.id, ChatRole::Candidate, SIDE_CHAT_QUESTION);
        self.chat(anchored.id, ChatRole::Agent, SIDE_CHAT_ANSWER);
        let course_only = self.store.start_side_chat(self.course, None).unwrap();
        self.chat(course_only.id, ChatRole::Candidate, COURSE_CHAT_QUESTION);
    }

    fn define(&self, version: Uuid, term: &str, definition: &str) {
        self.store
            .add_glossary_entry(&NewGlossaryEntry {
                course: self.course,
                term: term.to_owned(),
                definition: definition.to_owned(),
                anchor: Anchor {
                    version_id: version,
                    block: 0,
                    quote: "version".to_owned(),
                },
            })
            .unwrap();
    }

    fn chat(&self, chat: Uuid, role: ChatRole, text: &str) {
        self.store
            .add_chat_message(
                chat,
                &NewChatMessage {
                    role,
                    text: text.to_owned(),
                    state: MessageState::Complete,
                    citations: Vec::new(),
                    sources: Vec::new(),
                },
            )
            .unwrap();
    }

    async fn document(&self) -> ExportDocument {
        match self
            .core
            .handle(CoreRequest::GetExportDocument {
                course_id: self.course,
            })
            .await
        {
            Ok(CoreResponse::ExportDocument { document }) => document,
            other => panic!("expected the export document, got {other:?}"),
        }
    }

    async fn export(&self) -> ExportResult {
        match self
            .core
            .handle(CoreRequest::ExportPdf {
                course_id: self.course,
            })
            .await
        {
            Ok(CoreResponse::Exported { result }) => result,
            other => panic!("expected an export result, got {other:?}"),
        }
    }

    /// The version as the interface sees it, read back from the store.
    fn version_view(&self, id: Uuid) -> SectionVersion {
        let stored = self.store.version(id).unwrap().expect("a stored version");
        SectionVersion {
            id,
            style: stored.style,
            blocks: stored.blocks,
            citations: stored.citations,
            sources: stored.sources,
            written_without_search: stored.written_without_search,
            model: stored.model,
            created_at: stored.created_at,
        }
    }
}

#[tokio::test]
async fn the_document_carries_the_course_title_and_every_index_entry_in_index_order() {
    let harness = Harness::new();

    let document = harness.document().await;

    assert_eq!(document.course_id, harness.course);
    assert_eq!(document.title, TOPIC);
    let contents: Vec<(Uuid, ContentsLevel, u32, &str, bool)> = document
        .contents
        .iter()
        .map(|entry: &ContentsEntry| {
            (
                entry.node_id,
                entry.level,
                entry.depth,
                entry.title.as_str(),
                entry.included,
            )
        })
        .collect();
    assert_eq!(
        contents,
        vec![
            (
                CHAPTER_STRUCTURE,
                ContentsLevel::Chapter,
                0,
                "Earth's structure",
                true
            ),
            (
                LAYERS,
                ContentsLevel::Section,
                1,
                "Layers of the Earth",
                true
            ),
            (
                LITHOSPHERE,
                ContentsLevel::Section,
                1,
                "Lithosphere and asthenosphere",
                true
            ),
            (
                CHAPTER_MOTION,
                ContentsLevel::Chapter,
                0,
                "Plates in motion",
                true
            ),
            (
                WHAT_A_PLATE_IS,
                ContentsLevel::Section,
                1,
                "What a plate is",
                true
            ),
            (
                HOW_PLATES_ARE_MAPPED,
                ContentsLevel::DeepDive,
                2,
                "How plates are mapped",
                true
            ),
            (
                PLATE_THICKNESS,
                ContentsLevel::DeepDive,
                2,
                "Plate thickness",
                false
            ),
            (
                MICROPLATES,
                ContentsLevel::DeepDive,
                2,
                "Microplates",
                false
            ),
            (
                WHAT_DRIVES_PLATES,
                ContentsLevel::Section,
                1,
                "What drives the plates",
                false
            ),
            (
                CHAPTER_BOUNDARIES,
                ContentsLevel::Chapter,
                0,
                "Where plates meet",
                false
            ),
            (
                KINDS_OF_BOUNDARY,
                ContentsLevel::Section,
                1,
                "Kinds of boundary",
                false
            ),
            (
                SUBDUCTION_ZONES,
                ContentsLevel::Section,
                1,
                "Subduction zones",
                false
            ),
        ]
    );
}

#[tokio::test]
async fn sections_appear_in_index_order_with_the_deep_dive_under_its_section_each_in_its_current_version()
 {
    let harness = Harness::new();

    let document = harness.document().await;

    assert_eq!(
        document.sections,
        vec![
            ExportSection {
                node_id: LAYERS,
                kind: SectionKind::BroadPass,
                depth: 1,
                title: "Layers of the Earth".to_owned(),
                summary: "Crust, mantle and core, and what sets them apart.".to_owned(),
                version: harness.version_view(LAYERS_V1),
            },
            ExportSection {
                node_id: LITHOSPHERE,
                kind: SectionKind::BroadPass,
                depth: 1,
                title: "Lithosphere and asthenosphere".to_owned(),
                summary: "The rigid shell and the soft layer it rides on.".to_owned(),
                version: harness.version_view(LITHOSPHERE_V2),
            },
            ExportSection {
                node_id: WHAT_A_PLATE_IS,
                kind: SectionKind::BroadPass,
                depth: 1,
                title: "What a plate is".to_owned(),
                summary: "A rigid piece of the lithosphere that moves as one.".to_owned(),
                version: harness.version_view(WHAT_A_PLATE_IS_V1),
            },
            ExportSection {
                node_id: HOW_PLATES_ARE_MAPPED,
                kind: SectionKind::DeepDive,
                depth: 2,
                title: "How plates are mapped".to_owned(),
                summary: "Earthquake belts and satellite positions draw the edges.".to_owned(),
                version: harness.version_view(HOW_PLATES_ARE_MAPPED_V1),
            },
        ]
    );
    let layers = &document.sections[0].version;
    assert_eq!(layers.blocks, vec![prose(0, &text_of(LAYERS_V1))]);
    assert_eq!(layers.citations, layers_citations());
    assert_eq!(layers.sources, layers_sources());
    let lithosphere = &document.sections[1].version;
    assert_eq!(lithosphere.id, LITHOSPHERE_V2);
    assert_eq!(lithosphere.style, VersionStyle::Simpler);
    assert_eq!(lithosphere.blocks, vec![prose(0, &text_of(LITHOSPHERE_V2))]);
}

#[tokio::test]
async fn the_version_switched_back_to_is_the_one_exported() {
    let harness = Harness::new();
    harness
        .store
        .set_current_version(LITHOSPHERE, LITHOSPHERE_V1)
        .unwrap();

    let document = harness.document().await;

    let lithosphere = document
        .sections
        .iter()
        .find(|section| section.node_id == LITHOSPHERE)
        .expect("the lithosphere is exported");
    assert_eq!(lithosphere.version.id, LITHOSPHERE_V1);
    assert_eq!(
        lithosphere.version.blocks,
        vec![prose(0, &text_of(LITHOSPHERE_V1))]
    );
    let json = serde_json::to_string(&document).unwrap();
    assert!(
        !json.contains(&text_of(LITHOSPHERE_V2)),
        "the version not current is not exported"
    );
}

#[tokio::test]
async fn unopened_entries_appear_only_in_the_contents() {
    let harness = Harness::new();

    let document = harness.document().await;

    let unopened = [
        PLATE_THICKNESS,
        MICROPLATES,
        WHAT_DRIVES_PLATES,
        KINDS_OF_BOUNDARY,
        SUBDUCTION_ZONES,
    ];
    for node in unopened {
        assert!(
            !document
                .sections
                .iter()
                .any(|section| section.node_id == node),
            "{node} has no section in the document"
        );
        let entry = document
            .contents
            .iter()
            .find(|entry| entry.node_id == node)
            .expect("every unopened entry is in the contents");
        assert!(!entry.included, "{node} is listed by its title only");
    }
}

#[tokio::test]
async fn the_glossary_is_the_appendix_with_each_term_its_definition_and_section() {
    let harness = Harness::new();

    let document = harness.document().await;

    let glossary: Vec<(&str, &str, Uuid, &str)> = document
        .glossary
        .iter()
        .map(|entry| {
            (
                entry.term.as_str(),
                entry.definition.as_str(),
                entry.section_id,
                entry.section_title.as_str(),
            )
        })
        .collect();
    assert_eq!(
        glossary,
        vec![
            (
                "crust",
                "The thin rocky skin of the Earth.",
                LAYERS,
                "Layers of the Earth"
            ),
            (
                "tectonic plate",
                "A rigid piece of the lithosphere that moves as one.",
                WHAT_A_PLATE_IS,
                "What a plate is"
            ),
        ]
    );
    let json = serde_json::to_string(&document).unwrap();
    let at = |key: &str| {
        json.find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("no {key} in {json}"))
    };
    assert!(
        at("glossary") > at("contents") && at("glossary") > at("sections"),
        "the glossary comes after the contents and the sections: {json}"
    );
}

#[tokio::test]
async fn no_side_chat_text_appears_anywhere() {
    let harness = Harness::new();

    let document = harness.document().await;

    let json = serde_json::to_string(&document).unwrap();
    for text in [SIDE_CHAT_QUESTION, SIDE_CHAT_ANSWER, COURSE_CHAT_QUESTION] {
        assert!(!json.contains(text), "side-chat text exported: {text}");
    }
}

#[tokio::test]
async fn building_the_document_makes_no_model_request() {
    let harness = Harness::new();

    harness.document().await;

    assert!(harness.provider.requests().is_empty());
}

#[tokio::test]
async fn an_unknown_course_is_not_found() {
    let harness = Harness::new();

    let answer = harness
        .core
        .handle(CoreRequest::GetExportDocument {
            course_id: Uuid::from_u128(0xdead),
        })
        .await;

    assert!(
        matches!(answer, Err(CoreError::NotFound { .. })),
        "expected not found, got {answer:?}"
    );
}

#[tokio::test]
async fn export_pdf_passes_the_course_to_the_exporter_and_returns_the_saved_path() {
    let harness = Harness::exporting(PdfOutcome::Saved(PathBuf::from(
        "/Users/candidate/Documents/Plate tectonics.pdf",
    )));

    let result = harness.export().await;

    assert_eq!(
        result,
        ExportResult::Saved {
            path: "/Users/candidate/Documents/Plate tectonics.pdf".to_owned()
        }
    );
    let requests = harness.pdf.requests();
    assert_eq!(requests.len(), 1, "one export: {requests:?}");
    assert_eq!(requests[0].0, harness.course);
    assert_eq!(requests[0].1, "Plate tectonics.pdf");
    assert!(harness.provider.requests().is_empty());
}

#[tokio::test]
async fn export_pdf_maps_a_dismissed_save_dialog_to_cancelled() {
    let harness = Harness::exporting(PdfOutcome::Cancelled);

    assert_eq!(harness.export().await, ExportResult::Cancelled);
    assert_eq!(harness.pdf.requests().len(), 1);
}

#[tokio::test]
async fn export_pdf_maps_a_failed_print_to_failed_with_its_reason() {
    let harness = Harness::exporting(PdfOutcome::Failed("the disk is full".to_owned()));

    assert_eq!(
        harness.export().await,
        ExportResult::Failed {
            reason: "the disk is full".to_owned()
        }
    );
    assert_eq!(harness.pdf.requests().len(), 1);
}

#[tokio::test]
async fn export_pdf_of_an_unknown_course_reaches_no_exporter() {
    let harness = Harness::new();

    let answer = harness
        .core
        .handle(CoreRequest::ExportPdf {
            course_id: Uuid::from_u128(0xdead),
        })
        .await;

    assert!(
        matches!(answer, Err(CoreError::NotFound { .. })),
        "expected not found, got {answer:?}"
    );
    assert_eq!(harness.pdf.requests(), Vec::new());
}
