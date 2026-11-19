//! A sample course as typed values, shared by the tests of later features:
//! a ladder, a candidate profile, an index of three chapters with a deep dive,
//! a valid course map with its conventions, and one section record for each
//! section read.
//!
//! Ids are fixed so tests can name nodes and versions directly.

use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::{Concept, CourseMap, MapEntry};
use atlas_core::domain::ladder::{Ladder, Rung, RungId};
use atlas_core::domain::profile::{CandidateProfile, Knowledge, Misconception, RungKnowledge};
use atlas_core::domain::record::{
    FixedEntry, Promise, RecordStatus, SectionRecord, StoredRecord, TaughtConcept,
};
use atlas_core::index_tree::{IndexTree, TreeChapter, TreeSection};
use atlas_core::protocol::Goal;
use uuid::Uuid;

pub const TOPIC: &str = "Plate tectonics";

// Chapters and the sections under them, in index order.
pub const CHAPTER_STRUCTURE: Uuid = Uuid::from_u128(0x100);
pub const LAYERS: Uuid = Uuid::from_u128(0x101);
pub const LITHOSPHERE: Uuid = Uuid::from_u128(0x102);
pub const CHAPTER_MOTION: Uuid = Uuid::from_u128(0x200);
pub const WHAT_A_PLATE_IS: Uuid = Uuid::from_u128(0x201);
/// The deep dive under "What a plate is", three children.
pub const HOW_PLATES_ARE_MAPPED: Uuid = Uuid::from_u128(0x211);
pub const PLATE_THICKNESS: Uuid = Uuid::from_u128(0x212);
pub const MICROPLATES: Uuid = Uuid::from_u128(0x213);
pub const WHAT_DRIVES_PLATES: Uuid = Uuid::from_u128(0x202);
pub const CHAPTER_BOUNDARIES: Uuid = Uuid::from_u128(0x300);
pub const KINDS_OF_BOUNDARY: Uuid = Uuid::from_u128(0x301);
pub const SUBDUCTION_ZONES: Uuid = Uuid::from_u128(0x302);

// The current version of each section read.
pub const LAYERS_V1: Uuid = Uuid::from_u128(0x1101);
pub const LITHOSPHERE_V1: Uuid = Uuid::from_u128(0x1102);
pub const WHAT_A_PLATE_IS_V1: Uuid = Uuid::from_u128(0x1201);
pub const HOW_PLATES_ARE_MAPPED_V1: Uuid = Uuid::from_u128(0x1211);

/// One section record, kept with the version that wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleRecord {
    pub section: Uuid,
    pub version: Uuid,
    pub record: StoredRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleCourse {
    pub topic: String,
    pub ladder: Ladder,
    pub profile: CandidateProfile,
    pub index: IndexTree,
    pub map: CourseMap,
    pub conventions: Vec<Convention>,
    /// One per section read, in index order.
    pub records: Vec<SampleRecord>,
}

pub fn sample_course() -> SampleCourse {
    SampleCourse {
        topic: TOPIC.to_owned(),
        ladder: ladder(),
        profile: profile(),
        index: index(),
        map: map(),
        conventions: conventions(),
        records: records(),
    }
}

fn texts(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
}

fn ladder() -> Ladder {
    let concepts = [
        "Earth's layers",
        "Lithosphere and asthenosphere",
        "Tectonic plates",
        "Mantle convection",
        "Plate boundaries",
        "Seafloor spreading",
        "Subduction",
        "Earthquakes and volcanoes at boundaries",
    ];
    Ladder {
        rungs: (1..)
            .zip(concepts)
            .map(|(id, concept)| Rung {
                id: RungId(id),
                concept: concept.to_owned(),
            })
            .collect(),
    }
}

fn profile() -> CandidateProfile {
    let state = |rung: u32| match rung {
        1 | 2 => Knowledge::Known,
        3 => Knowledge::Shaky,
        _ => Knowledge::Unknown,
    };
    CandidateProfile {
        goal: Goal::WorkingKnowledge,
        rungs: (1..=8)
            .map(|rung| RungKnowledge {
                rung: RungId(rung),
                state: state(rung),
            })
            .collect(),
        misconceptions: vec![Misconception {
            rung: RungId(4),
            text: "The mantle is a sea of liquid rock.".to_owned(),
        }],
    }
}

fn section(id: Uuid, title: &str, line: &str, read: bool) -> TreeSection {
    TreeSection {
        id,
        title: title.to_owned(),
        line: line.to_owned(),
        read,
        children: Vec::new(),
    }
}

fn index() -> IndexTree {
    let mut what_a_plate_is = section(
        WHAT_A_PLATE_IS,
        "What a plate is",
        "A rigid piece of the lithosphere that moves as one.",
        true,
    );
    what_a_plate_is.children = vec![
        section(
            HOW_PLATES_ARE_MAPPED,
            "How plates are mapped",
            "Earthquake belts and satellite positions draw the edges.",
            true,
        ),
        section(
            PLATE_THICKNESS,
            "Plate thickness",
            "Why oceanic plates are thin and continental ones thick.",
            false,
        ),
        section(
            MICROPLATES,
            "Microplates",
            "Small plates caught between large ones.",
            false,
        ),
    ];
    IndexTree {
        chapters: vec![
            TreeChapter {
                id: CHAPTER_STRUCTURE,
                title: "Earth's structure".to_owned(),
                sections: vec![
                    section(
                        LAYERS,
                        "Layers of the Earth",
                        "Crust, mantle and core, and what sets them apart.",
                        true,
                    ),
                    section(
                        LITHOSPHERE,
                        "Lithosphere and asthenosphere",
                        "The rigid shell and the soft layer it rides on.",
                        true,
                    ),
                ],
            },
            TreeChapter {
                id: CHAPTER_MOTION,
                title: "Plates in motion".to_owned(),
                sections: vec![
                    what_a_plate_is,
                    section(
                        WHAT_DRIVES_PLATES,
                        "What drives the plates",
                        "Convection in the mantle and the pull of sinking slabs.",
                        false,
                    ),
                ],
            },
            TreeChapter {
                id: CHAPTER_BOUNDARIES,
                title: "Where plates meet".to_owned(),
                sections: vec![
                    section(
                        KINDS_OF_BOUNDARY,
                        "Kinds of boundary",
                        "Plates pull apart, collide or slide past each other.",
                        false,
                    ),
                    section(
                        SUBDUCTION_ZONES,
                        "Subduction zones",
                        "Where one plate sinks beneath another.",
                        false,
                    ),
                ],
            },
        ],
    }
}

fn map() -> CourseMap {
    let concept = |name: &str, rung: Option<u32>| Concept {
        name: name.to_owned(),
        rung: rung.map(RungId),
    };
    let entry = |section: Uuid, introduces: &[&str], relies_on: &[&str]| MapEntry {
        section,
        introduces: texts(introduces),
        relies_on: texts(relies_on),
    };
    CourseMap {
        concepts: vec![
            concept("crust", Some(1)),
            concept("mantle", Some(1)),
            concept("core", Some(1)),
            concept("lithosphere", Some(2)),
            concept("asthenosphere", Some(2)),
            concept("tectonic plate", Some(3)),
            concept("plate velocity", None),
            concept("plate thickness", None),
            concept("microplate", None),
            concept("mantle convection", Some(4)),
            concept("divergent boundary", Some(5)),
            concept("convergent boundary", Some(5)),
            concept("transform boundary", Some(5)),
            concept("seafloor spreading", Some(6)),
            concept("subduction", Some(7)),
        ],
        entries: vec![
            entry(LAYERS, &["crust", "mantle", "core"], &[]),
            entry(
                LITHOSPHERE,
                &["lithosphere", "asthenosphere"],
                &["crust", "mantle"],
            ),
            entry(
                WHAT_A_PLATE_IS,
                &["tectonic plate"],
                &["lithosphere", "asthenosphere"],
            ),
            entry(
                HOW_PLATES_ARE_MAPPED,
                &["plate velocity"],
                &["tectonic plate"],
            ),
            entry(
                PLATE_THICKNESS,
                &["plate thickness"],
                &["lithosphere", "tectonic plate"],
            ),
            entry(MICROPLATES, &["microplate"], &["tectonic plate"]),
            entry(
                WHAT_DRIVES_PLATES,
                &["mantle convection"],
                &["mantle", "asthenosphere", "tectonic plate"],
            ),
            entry(
                KINDS_OF_BOUNDARY,
                &[
                    "divergent boundary",
                    "convergent boundary",
                    "transform boundary",
                    "seafloor spreading",
                ],
                &["tectonic plate", "mantle convection"],
            ),
            entry(
                SUBDUCTION_ZONES,
                &["subduction"],
                &["convergent boundary", "crust"],
            ),
        ],
    }
}

fn conventions() -> Vec<Convention> {
    let convention = |kind, key: &str, meaning: &str, origin| Convention {
        kind,
        key: key.to_owned(),
        meaning: meaning.to_owned(),
        origin,
    };
    vec![
        convention(
            ConventionKind::Terminology,
            "plate",
            "a rigid piece of the lithosphere that moves as one",
            ConventionOrigin::Plan,
        ),
        convention(
            ConventionKind::Notation,
            "v",
            "plate velocity, in centimetres per year",
            ConventionOrigin::Plan,
        ),
        convention(
            ConventionKind::RunningExample,
            "the Atlantic",
            "the widening Atlantic Ocean, the example of plates moving apart",
            ConventionOrigin::Plan,
        ),
        convention(
            ConventionKind::Terminology,
            "asthenosphere",
            "the weak, slowly flowing upper mantle beneath the lithosphere",
            ConventionOrigin::SectionVersion {
                version: LITHOSPHERE_V1,
            },
        ),
    ]
}

fn taught(name: &str, line: &str) -> TaughtConcept {
    TaughtConcept {
        name: name.to_owned(),
        line: Some(line.to_owned()),
    }
}

fn fixed(key: &str, meaning: &str) -> FixedEntry {
    FixedEntry {
        key: key.to_owned(),
        meaning: meaning.to_owned(),
    }
}

fn promise(section: &str, what: &str) -> Promise {
    Promise {
        section: section.to_owned(),
        what: what.to_owned(),
    }
}

fn records() -> Vec<SampleRecord> {
    let confirmed = |section, version, record| SampleRecord {
        section,
        version,
        record: StoredRecord {
            record,
            status: RecordStatus::Confirmed,
        },
    };
    vec![
        confirmed(
            LAYERS,
            LAYERS_V1,
            SectionRecord {
                summary: Some(
                    "The Earth is built in layers: a thin crust, a thick mantle and a \
                     metal core. They differ in what they are made of."
                        .to_owned(),
                ),
                concepts: vec![
                    taught("crust", "The thin rocky skin we live on."),
                    taught("mantle", "Hot rock below the crust that creeps over time."),
                    taught("core", "The iron centre, liquid outside and solid inside."),
                ],
                notation: Vec::new(),
                terms: vec![fixed("crust", "the thin rocky outer layer of the Earth")],
                examples: texts(&["a peach: skin, flesh and stone"]),
                promises: vec![promise(
                    "1.2",
                    "how the outer layers split by strength rather than by composition",
                )],
            },
        ),
        confirmed(
            LITHOSPHERE,
            LITHOSPHERE_V1,
            SectionRecord {
                summary: Some(
                    "By strength, the crust and the top of the mantle form one rigid \
                     shell, the lithosphere. It rides on the asthenosphere, which flows \
                     slowly."
                        .to_owned(),
                ),
                concepts: vec![
                    taught(
                        "lithosphere",
                        "The rigid shell: crust plus uppermost mantle.",
                    ),
                    taught("asthenosphere", "The soft layer below, solid but flowing."),
                ],
                notation: Vec::new(),
                terms: vec![fixed(
                    "asthenosphere",
                    "the weak, slowly flowing upper mantle beneath the lithosphere",
                )],
                examples: texts(&["ice floes on a pond"]),
                promises: vec![promise("2.1", "what a single plate is")],
            },
        ),
        confirmed(
            WHAT_A_PLATE_IS,
            WHAT_A_PLATE_IS_V1,
            SectionRecord {
                summary: Some(
                    "The lithosphere is broken into plates that move as rigid pieces. \
                     They move a few centimetres a year."
                        .to_owned(),
                ),
                concepts: vec![taught(
                    "tectonic plate",
                    "A rigid piece of the lithosphere that moves as one.",
                )],
                notation: vec![fixed("v", "plate velocity, in centimetres per year")],
                terms: Vec::new(),
                examples: texts(&["the Atlantic widening by about 2.5 cm a year"]),
                promises: vec![promise("2.2", "what makes the plates move")],
            },
        ),
        SampleRecord {
            section: HOW_PLATES_ARE_MAPPED,
            version: HOW_PLATES_ARE_MAPPED_V1,
            // Predicted from the map entry: the record failed validation twice.
            record: StoredRecord {
                record: SectionRecord {
                    summary: None,
                    concepts: vec![TaughtConcept {
                        name: "plate velocity".to_owned(),
                        line: None,
                    }],
                    notation: Vec::new(),
                    terms: Vec::new(),
                    examples: Vec::new(),
                    promises: Vec::new(),
                },
                status: RecordStatus::Predicted,
            },
        },
    ]
}
