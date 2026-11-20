//! The course map's rules on the sample course and on maps changed here:
//! validation, deep-dive extension, lookups, conventions keeping their first
//! fixing, and the record a map entry predicts.

mod support;

use atlas_core::course_map::{
    ConventionConflict, ConventionMerge, FaultKind, MapFault, entry, extend, introducer,
    merge_conventions, predicted_record, prerequisite_sections, validate,
};
use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::{Concept, CourseMap, MapEntry};
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::record::{RecordStatus, SectionRecord, StoredRecord, TaughtConcept};
use atlas_core::index_tree::{IndexTree, TreeSection, in_index_order, insert_children};
use support::sample_course::{
    HOW_PLATES_ARE_MAPPED, KINDS_OF_BOUNDARY, LAYERS, LITHOSPHERE, MICROPLATES, SUBDUCTION_ZONES,
    SampleCourse, WHAT_A_PLATE_IS, WHAT_DRIVES_PLATES, sample_course,
};
use uuid::Uuid;

/// The deep dive proposed under "Kinds of boundary", in order.
const RIDGES: Uuid = Uuid::from_u128(0x311);
const TRENCHES: Uuid = Uuid::from_u128(0x312);
const TRANSFORM_FAULTS: Uuid = Uuid::from_u128(0x313);

fn order(index: &IndexTree) -> Vec<Uuid> {
    in_index_order(index)
        .iter()
        .map(|entry| entry.section.id)
        .collect()
}

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
}

fn map_entry(section: Uuid, introduces: &[&str], relies_on: &[&str]) -> MapEntry {
    MapEntry {
        section,
        introduces: names(introduces),
        relies_on: names(relies_on),
    }
}

fn entry_mut(map: &mut CourseMap, section: Uuid) -> &mut MapEntry {
    map.entries
        .iter_mut()
        .find(|entry| entry.section == section)
        .expect("the sample map has an entry for the section")
}

fn fault(section: Uuid, concept: Option<&str>, kind: FaultKind) -> MapFault {
    MapFault {
        section,
        concept: concept.map(str::to_owned),
        kind,
    }
}

fn faults_of(course: &SampleCourse) -> Vec<MapFault> {
    validate(&course.map, &order(&course.index), &course.ladder)
}

#[test]
fn the_sample_map_is_valid() {
    let course = sample_course();
    assert_eq!(faults_of(&course), Vec::new());
}

#[test]
fn a_concept_introduced_after_the_section_relying_on_it_is_a_fault() {
    let mut course = sample_course();
    entry_mut(&mut course.map, WHAT_DRIVES_PLATES)
        .introduces
        .retain(|concept| concept != "mantle convection");
    entry_mut(&mut course.map, SUBDUCTION_ZONES)
        .introduces
        .push("mantle convection".to_owned());
    assert_eq!(
        faults_of(&course),
        vec![fault(
            KINDS_OF_BOUNDARY,
            Some("mantle convection"),
            FaultKind::NotIntroducedEarlier
        )]
    );
}

#[test]
fn a_relied_on_concept_no_section_introduces_is_a_fault() {
    let mut course = sample_course();
    entry_mut(&mut course.map, WHAT_DRIVES_PLATES)
        .introduces
        .retain(|concept| concept != "mantle convection");
    assert_eq!(
        faults_of(&course),
        vec![fault(
            KINDS_OF_BOUNDARY,
            Some("mantle convection"),
            FaultKind::NotIntroducedEarlier
        )]
    );
}

#[test]
fn relying_on_a_concept_the_section_itself_introduces_is_a_fault() {
    let mut course = sample_course();
    entry_mut(&mut course.map, LAYERS)
        .relies_on
        .push("core".to_owned());
    assert_eq!(
        faults_of(&course),
        vec![fault(LAYERS, Some("core"), FaultKind::NotIntroducedEarlier)]
    );
}

#[test]
fn a_section_without_an_entry_is_a_fault() {
    let mut course = sample_course();
    course
        .map
        .entries
        .retain(|entry| entry.section != MICROPLATES);
    assert_eq!(
        faults_of(&course),
        vec![fault(MICROPLATES, None, FaultKind::MissingEntry)]
    );
}

#[test]
fn a_concept_naming_a_rung_the_ladder_lacks_is_a_fault() {
    let mut course = sample_course();
    let subduction = course
        .map
        .concepts
        .iter_mut()
        .find(|concept| concept.name == "subduction")
        .expect("the sample map has the concept");
    subduction.rung = Some(RungId(99));
    assert_eq!(
        faults_of(&course),
        vec![fault(
            SUBDUCTION_ZONES,
            Some("subduction"),
            FaultKind::UnknownRung
        )]
    );
}

#[test]
fn an_empty_concept_name_is_a_fault() {
    let mut course = sample_course();
    entry_mut(&mut course.map, LITHOSPHERE)
        .introduces
        .push("  ".to_owned());
    assert_eq!(
        faults_of(&course),
        vec![fault(LITHOSPHERE, Some("  "), FaultKind::EmptyName)]
    );
}

#[test]
fn several_faults_are_all_reported() {
    let mut course = sample_course();
    course
        .map
        .entries
        .retain(|entry| entry.section != MICROPLATES);
    entry_mut(&mut course.map, WHAT_DRIVES_PLATES)
        .introduces
        .retain(|concept| concept != "mantle convection");
    entry_mut(&mut course.map, SUBDUCTION_ZONES)
        .introduces
        .push("mantle convection".to_owned());
    course
        .map
        .concepts
        .iter_mut()
        .filter(|concept| concept.name == "subduction")
        .for_each(|concept| concept.rung = Some(RungId(99)));

    let faults = faults_of(&course);
    let expected = [
        fault(MICROPLATES, None, FaultKind::MissingEntry),
        fault(
            KINDS_OF_BOUNDARY,
            Some("mantle convection"),
            FaultKind::NotIntroducedEarlier,
        ),
        fault(SUBDUCTION_ZONES, Some("subduction"), FaultKind::UnknownRung),
    ];
    assert_eq!(faults.len(), expected.len(), "{faults:?}");
    for expected in expected {
        assert!(faults.contains(&expected), "{expected:?} in {faults:?}");
    }
}

#[test]
fn a_fault_reads_as_a_sentence_naming_section_and_concept() {
    let line = fault(
        KINDS_OF_BOUNDARY,
        Some("mantle convection"),
        FaultKind::NotIntroducedEarlier,
    )
    .to_string();
    assert!(line.contains(&KINDS_OF_BOUNDARY.to_string()), "{line}");
    assert!(line.contains("\"mantle convection\""), "{line}");
}

#[test]
fn a_concept_introduced_twice_is_valid_and_introduced_by_the_earlier_section() {
    let mut course = sample_course();
    entry_mut(&mut course.map, LITHOSPHERE)
        .introduces
        .push("tectonic plate".to_owned());
    entry_mut(&mut course.map, SUBDUCTION_ZONES)
        .introduces
        .push("crust".to_owned());
    // The entries' own order says nothing; the index order decides.
    course.map.entries.reverse();
    let order = order(&course.index);

    assert_eq!(faults_of(&course), Vec::new());
    assert_eq!(
        introducer(&course.map, &order, "tectonic plate"),
        Some(LITHOSPHERE)
    );
    assert_eq!(introducer(&course.map, &order, "crust"), Some(LAYERS));
}

#[test]
fn lookups_find_an_entry_an_introducer_and_the_prerequisite_sections() {
    let course = sample_course();
    let order = order(&course.index);

    assert_eq!(
        entry(&course.map, LITHOSPHERE),
        Some(&map_entry(
            LITHOSPHERE,
            &["lithosphere", "asthenosphere"],
            &["crust", "mantle"],
        ))
    );
    assert_eq!(entry(&course.map, Uuid::from_u128(0xdead)), None);

    assert_eq!(
        introducer(&course.map, &order, "plate velocity"),
        Some(HOW_PLATES_ARE_MAPPED)
    );
    assert_eq!(introducer(&course.map, &order, "plate tectonics"), None);

    assert_eq!(
        prerequisite_sections(&course.map, &order, WHAT_DRIVES_PLATES),
        vec![LAYERS, LITHOSPHERE, WHAT_A_PLATE_IS]
    );
    // Each section once, in index order.
    assert_eq!(
        prerequisite_sections(&course.map, &order, LITHOSPHERE),
        vec![LAYERS]
    );
    assert_eq!(
        prerequisite_sections(&course.map, &order, SUBDUCTION_ZONES),
        vec![LAYERS, KINDS_OF_BOUNDARY]
    );
    assert!(prerequisite_sections(&course.map, &order, LAYERS).is_empty());
}

/// The sample index with three children proposed under "Kinds of boundary",
/// and its index order.
fn deepened(course: &SampleCourse) -> Vec<Uuid> {
    let child = |id, title: &str| TreeSection {
        id,
        title: title.to_owned(),
        line: String::new(),
        read: false,
        children: Vec::new(),
    };
    let mut index = course.index.clone();
    insert_children(
        &mut index,
        KINDS_OF_BOUNDARY,
        vec![
            child(RIDGES, "Mid-ocean ridges"),
            child(TRENCHES, "Trenches"),
            child(TRANSFORM_FAULTS, "Transform faults"),
        ],
    )
    .expect("the deep dive fits the index");
    order(&index)
}

fn proposals(ridges: &[&str], trenches: &[&str], transform_faults: &[&str]) -> CourseMap {
    CourseMap {
        concepts: vec![
            Concept {
                name: "mid-ocean ridge".to_owned(),
                rung: Some(RungId(6)),
            },
            Concept {
                name: "trench".to_owned(),
                rung: Some(RungId(7)),
            },
            Concept {
                name: "transform fault".to_owned(),
                rung: None,
            },
        ],
        entries: vec![
            map_entry(RIDGES, &["mid-ocean ridge"], ridges),
            map_entry(TRENCHES, &["trench"], trenches),
            map_entry(TRANSFORM_FAULTS, &["transform fault"], transform_faults),
        ],
    }
}

#[test]
fn proposals_relying_on_the_parent_an_earlier_sibling_and_an_earlier_chapter_extend_the_map() {
    let course = sample_course();
    let order = deepened(&course);
    let proposed = proposals(
        // The parent's concept, and one from the first chapter.
        &["divergent boundary", "lithosphere"],
        // An earlier sibling's concept.
        &["mid-ocean ridge", "convergent boundary"],
        &["transform boundary", "trench"],
    );

    let extended =
        extend(&course.map, &order, &proposed, &course.ladder).expect("the proposals are valid");

    let existing = course.map.entries.len();
    assert_eq!(
        serde_json::to_vec(&extended.entries[..existing]).unwrap(),
        serde_json::to_vec(&course.map.entries).unwrap(),
        "the existing entries are unchanged"
    );
    assert_eq!(extended.entries[existing..], proposed.entries[..]);
    assert_eq!(
        serde_json::to_vec(&extended.concepts[..course.map.concepts.len()]).unwrap(),
        serde_json::to_vec(&course.map.concepts).unwrap()
    );
    for concept in &proposed.concepts {
        assert!(extended.concepts.contains(concept), "{concept:?}");
    }
    assert_eq!(entry(&extended, TRENCHES), Some(&proposed.entries[1]));
    assert_eq!(introducer(&extended, &order, "trench"), Some(TRENCHES));
    assert_eq!(
        prerequisite_sections(&extended, &order, RIDGES),
        vec![LITHOSPHERE, KINDS_OF_BOUNDARY]
    );
    assert_eq!(validate(&extended, &order, &course.ladder), Vec::new());
}

#[test]
fn a_proposal_relying_on_a_later_siblings_concept_is_a_fault() {
    let course = sample_course();
    let order = deepened(&course);
    let proposed = proposals(&["divergent boundary"], &["transform fault"], &[]);
    assert_eq!(
        extend(&course.map, &order, &proposed, &course.ladder),
        Err(vec![fault(
            TRENCHES,
            Some("transform fault"),
            FaultKind::NotIntroducedEarlier
        )])
    );
}

#[test]
fn a_proposal_relying_on_a_later_broad_pass_sections_concept_is_a_fault() {
    let course = sample_course();
    let order = deepened(&course);
    let proposed = proposals(&[], &[], &["subduction"]);
    assert_eq!(
        extend(&course.map, &order, &proposed, &course.ladder),
        Err(vec![fault(
            TRANSFORM_FAULTS,
            Some("subduction"),
            FaultKind::NotIntroducedEarlier
        )])
    );
}

#[test]
fn a_proposed_child_without_an_entry_or_with_an_unknown_rung_is_a_fault() {
    let course = sample_course();
    let order = deepened(&course);
    let mut proposed = proposals(&[], &[], &[]);
    proposed
        .entries
        .retain(|entry| entry.section != TRANSFORM_FAULTS);
    proposed.concepts[1].rung = Some(RungId(42));

    let faults = extend(&course.map, &order, &proposed, &course.ladder)
        .expect_err("the proposals are faulty");
    let expected = [
        fault(TRENCHES, Some("trench"), FaultKind::UnknownRung),
        fault(TRANSFORM_FAULTS, None, FaultKind::MissingEntry),
    ];
    assert_eq!(faults.len(), expected.len(), "{faults:?}");
    for expected in expected {
        assert!(faults.contains(&expected), "{expected:?} in {faults:?}");
    }
}

fn convention(kind: ConventionKind, key: &str, meaning: &str) -> Convention {
    Convention {
        kind,
        key: key.to_owned(),
        meaning: meaning.to_owned(),
        origin: ConventionOrigin::SectionVersion {
            version: Uuid::from_u128(0x9999),
        },
    }
}

fn fixed(course: &SampleCourse, key: &str) -> Convention {
    course
        .conventions
        .iter()
        .find(|convention| convention.key == key)
        .cloned()
        .expect("the sample course fixed the convention")
}

#[test]
fn a_new_term_is_accepted() {
    let course = sample_course();
    let slab = convention(
        ConventionKind::Terminology,
        "slab",
        "the part of a plate sinking into the mantle",
    );
    assert_eq!(
        merge_conventions(&course.conventions, std::slice::from_ref(&slab)),
        ConventionMerge {
            accepted: vec![slab],
            conflicts: Vec::new(),
        }
    );
}

#[test]
fn a_fixed_symbol_with_another_meaning_is_a_conflict_and_the_first_stays() {
    let course = sample_course();
    let volume = convention(ConventionKind::Notation, "v", "volume of magma");
    assert_eq!(
        merge_conventions(&course.conventions, std::slice::from_ref(&volume)),
        ConventionMerge {
            accepted: Vec::new(),
            conflicts: vec![ConventionConflict {
                fixed: fixed(&course, "v"),
                incoming: volume,
            }],
        }
    );
}

#[test]
fn keys_differing_only_in_case_or_surrounding_space_are_the_same_key() {
    let course = sample_course();
    let volume = convention(ConventionKind::Notation, "  V ", "volume of magma");
    let plate = convention(
        ConventionKind::Terminology,
        " Plate\t",
        "a rigid piece of the lithosphere that moves as one",
    );
    assert_eq!(
        merge_conventions(&course.conventions, &[volume.clone(), plate]),
        ConventionMerge {
            accepted: Vec::new(),
            conflicts: vec![ConventionConflict {
                fixed: fixed(&course, "v"),
                incoming: volume,
            }],
        }
    );
}

#[test]
fn an_identical_restatement_changes_nothing() {
    let course = sample_course();
    assert_eq!(
        merge_conventions(&course.conventions, &course.conventions),
        ConventionMerge::default()
    );
}

#[test]
fn the_first_incoming_fixing_holds_against_a_later_incoming_one() {
    let first = convention(ConventionKind::Terminology, "slab", "a sinking plate");
    let second = convention(ConventionKind::Terminology, "Slab", "a block of rock");
    assert_eq!(
        merge_conventions(&[], &[first.clone(), second.clone()]),
        ConventionMerge {
            accepted: vec![first.clone()],
            conflicts: vec![ConventionConflict {
                fixed: first,
                incoming: second,
            }],
        }
    );
}

#[test]
fn a_key_is_fixed_per_kind() {
    let course = sample_course();
    let term = convention(ConventionKind::Terminology, "v", "a valley-shaped rift");
    assert_eq!(
        merge_conventions(&course.conventions, std::slice::from_ref(&term)),
        ConventionMerge {
            accepted: vec![term],
            conflicts: Vec::new(),
        }
    );
}

#[test]
fn the_predicted_record_lists_the_planned_concepts_and_is_unconfirmed() {
    let course = sample_course();
    let boundaries = entry(&course.map, KINDS_OF_BOUNDARY).expect("the sample map has the entry");
    let planned = |name: &str| TaughtConcept {
        name: name.to_owned(),
        line: None,
    };
    assert_eq!(
        predicted_record(boundaries),
        StoredRecord {
            record: SectionRecord {
                summary: None,
                concepts: vec![
                    planned("divergent boundary"),
                    planned("convergent boundary"),
                    planned("transform boundary"),
                    planned("seafloor spreading"),
                ],
                notation: Vec::new(),
                terms: Vec::new(),
                examples: Vec::new(),
                promises: Vec::new(),
            },
            status: RecordStatus::Predicted,
        }
    );

    let mapped = course
        .records
        .iter()
        .find(|record| record.section == HOW_PLATES_ARE_MAPPED)
        .expect("the sample course has the predicted record");
    let mapped_entry = entry(&course.map, HOW_PLATES_ARE_MAPPED).unwrap();
    assert_eq!(predicted_record(mapped_entry), mapped.record);
}

#[test]
fn an_entry_for_no_section_or_a_second_entry_for_one_is_a_fault() {
    let mut course = sample_course();
    let stray = Uuid::from_u128(0xdead);
    course.map.entries.push(map_entry(stray, &[], &[]));
    course.map.entries.push(map_entry(LAYERS, &["crust"], &[]));
    assert_eq!(
        faults_of(&course),
        vec![
            fault(stray, None, FaultKind::UnknownSection),
            fault(LAYERS, None, FaultKind::DuplicateEntry),
        ]
    );
}

#[test]
fn a_proposal_for_a_section_the_map_already_has_is_a_fault() {
    let course = sample_course();
    let order = deepened(&course);
    let mut proposed = proposals(&[], &[], &[]);
    proposed
        .entries
        .push(map_entry(MICROPLATES, &["microplate"], &[]));
    assert_eq!(
        extend(&course.map, &order, &proposed, &course.ladder),
        Err(vec![fault(MICROPLATES, None, FaultKind::DuplicateEntry)])
    );
}
