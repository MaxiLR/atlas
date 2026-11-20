//! Section context selected by the core from the course memory: what a
//! section request carries, what it never carries, and the fixed shrink order
//! over budget.

mod support;

use std::collections::HashMap;

use atlas_core::domain::course_map::{Concept, MapEntry};
use atlas_core::domain::record::{
    FixedEntry, Promise, RecordStatus, SectionRecord, StoredRecord, TaughtConcept,
};
use atlas_core::harness::context::{
    Budget, CourseMemory, IndexKeys, PartKind, RECORD_CHARS, RECORD_ENTRIES, RECORD_LINE_CHARS,
    RECORD_SUMMARY_CHARS, SectionContext, Shrink, render_index, render_profile, section_context,
};
use atlas_core::index_tree::{IndexTree, TreeChapter, TreeSection, in_index_order};
use support::sample_course::{
    CHAPTER_STRUCTURE, HOW_PLATES_ARE_MAPPED, KINDS_OF_BOUNDARY, LAYERS, LITHOSPHERE, MICROPLATES,
    PLATE_THICKNESS, SUBDUCTION_ZONES, SampleCourse, WHAT_A_PLATE_IS, WHAT_DRIVES_PLATES,
    sample_course,
};
use uuid::Uuid;

// Two chapters added after the sample's three, so a section relies on a
// concept introduced four chapters earlier.
const CHAPTER_OCEANS: Uuid = Uuid::from_u128(0x400);
const MID_OCEAN_RIDGES: Uuid = Uuid::from_u128(0x401);
const CHAPTER_MOUNTAINS: Uuid = Uuid::from_u128(0x500);
const MOUNTAIN_BELTS: Uuid = Uuid::from_u128(0x501);

const LAYERS_SUMMARY: &str = "The Earth is built in layers";
const LITHOSPHERE_SUMMARY: &str = "By strength, the crust and the top of the mantle";
const WHAT_A_PLATE_IS_SUMMARY: &str = "The lithosphere is broken into plates";
const KINDS_SUMMARY: &str = "Plates meet in three ways: apart, together or alongside.";
const RIDGES_SUMMARY: &str = "Ridges are where plates part and new seafloor rises.";
const RIDGE_LINE: &str = "A long volcanic chain where new ocean floor is made.";

fn memory(course: &SampleCourse) -> CourseMemory {
    CourseMemory {
        ladder: course.ladder.clone(),
        profile: course.profile.clone(),
        index: course.index.clone(),
        map: course.map.clone(),
        conventions: course.conventions.clone(),
        records: course
            .records
            .iter()
            .map(|sample| (sample.section, sample.record.clone()))
            .collect(),
    }
}

fn sample_memory() -> CourseMemory {
    memory(&sample_course())
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

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
}

fn taught(name: &str, line: &str) -> TaughtConcept {
    TaughtConcept {
        name: name.to_owned(),
        line: Some(line.to_owned()),
    }
}

fn promise(section: &str, what: &str) -> Promise {
    Promise {
        section: section.to_owned(),
        what: what.to_owned(),
    }
}

fn confirmed(record: SectionRecord) -> StoredRecord {
    StoredRecord {
        record,
        status: RecordStatus::Confirmed,
    }
}

fn find_mut(sections: &mut [TreeSection], id: Uuid) -> Option<&mut TreeSection> {
    sections.iter_mut().find_map(|section| {
        if section.id == id {
            Some(section)
        } else {
            find_mut(&mut section.children, id)
        }
    })
}

fn set_read(memory: &mut CourseMemory, id: Uuid, read: bool) {
    let section = memory
        .index
        .chapters
        .iter_mut()
        .find_map(|chapter| find_mut(&mut chapter.sections, id))
        .expect("the section is in the index");
    section.read = read;
}

fn entry_mut(memory: &mut CourseMemory, section: Uuid) -> &mut MapEntry {
    memory
        .map
        .entries
        .iter_mut()
        .find(|entry| entry.section == section)
        .expect("the map has an entry for the section")
}

/// The sample course with "Kinds of boundary" read and two more chapters:
/// "Mid-ocean ridges" (4.1, read) and "Mountain belts" (5.1, unread), which
/// relies on "crust", introduced by "Layers of the Earth" (1.1).
fn long_memory() -> CourseMemory {
    let mut memory = sample_memory();
    set_read(&mut memory, KINDS_OF_BOUNDARY, true);
    memory.records.insert(
        KINDS_OF_BOUNDARY,
        confirmed(SectionRecord {
            summary: Some(KINDS_SUMMARY.to_owned()),
            concepts: vec![
                taught("divergent boundary", "Plates pull apart."),
                taught("convergent boundary", "Plates push together."),
                taught("transform boundary", "Plates slide past each other."),
                taught("seafloor spreading", "New crust forms where plates part."),
            ],
            notation: Vec::new(),
            terms: Vec::new(),
            examples: names(&["the San Andreas fault"]),
            promises: vec![promise("3.2", "where one plate dives under another")],
        }),
    );
    memory.index.chapters.push(TreeChapter {
        id: CHAPTER_OCEANS,
        title: "Ocean floors".to_owned(),
        sections: vec![section(
            MID_OCEAN_RIDGES,
            "Mid-ocean ridges",
            "Where new seafloor is made.",
            true,
        )],
    });
    memory.index.chapters.push(TreeChapter {
        id: CHAPTER_MOUNTAINS,
        title: "Building continents".to_owned(),
        sections: vec![section(
            MOUNTAIN_BELTS,
            "Mountain belts",
            "How collisions raise ranges.",
            false,
        )],
    });
    for name in ["mid-ocean ridge", "mountain belt"] {
        memory.map.concepts.push(Concept {
            name: name.to_owned(),
            rung: None,
        });
    }
    memory.map.entries.push(MapEntry {
        section: MID_OCEAN_RIDGES,
        introduces: names(&["mid-ocean ridge"]),
        relies_on: names(&["divergent boundary", "seafloor spreading"]),
    });
    memory.map.entries.push(MapEntry {
        section: MOUNTAIN_BELTS,
        introduces: names(&["mountain belt"]),
        relies_on: names(&["crust"]),
    });
    memory.records.insert(
        MID_OCEAN_RIDGES,
        confirmed(SectionRecord {
            summary: Some(RIDGES_SUMMARY.to_owned()),
            concepts: vec![taught("mid-ocean ridge", RIDGE_LINE)],
            notation: Vec::new(),
            terms: vec![FixedEntry {
                key: "ridge".to_owned(),
                meaning: "a mid-ocean ridge".to_owned(),
            }],
            examples: names(&["Iceland, a ridge above the sea"]),
            promises: vec![promise("5.1", "why mountains hold folded seafloor rock")],
        }),
    );
    memory
}

fn context(memory: &CourseMemory, node: Uuid) -> SectionContext {
    section_context(memory, node, Budget(usize::MAX)).expect("a section of the index")
}

fn part(context: &SectionContext, kind: PartKind) -> String {
    context
        .part(kind)
        .unwrap_or_else(|| panic!("the context has a {kind:?} part: {context:#?}"))
        .to_owned()
}

fn size(context: &SectionContext) -> usize {
    context.input().chars().count()
}

#[test]
fn every_entry_has_a_short_stable_key_mapped_back_to_its_node() {
    let index = sample_memory().index;

    let keys = IndexKeys::new(&index);

    let expected = [
        ("1", CHAPTER_STRUCTURE),
        ("1.1", LAYERS),
        ("1.2", LITHOSPHERE),
        ("2.1", WHAT_A_PLATE_IS),
        ("2.1.1", HOW_PLATES_ARE_MAPPED),
        ("2.1.2", PLATE_THICKNESS),
        ("2.1.3", MICROPLATES),
        ("2.2", WHAT_DRIVES_PLATES),
        ("3.1", KINDS_OF_BOUNDARY),
        ("3.2", SUBDUCTION_ZONES),
    ];
    for (key, node) in expected {
        assert_eq!(keys.key(node), Some(key), "{node}");
        assert_eq!(keys.node(key), Some(node), "{key}");
    }
    assert_eq!(keys.node("4.1"), None);
}

#[test]
fn the_index_rendering_names_every_title_with_its_key() {
    let rendered = render_index(&sample_memory().index);

    for line in [
        "1 Earth's structure",
        "1.1 Layers of the Earth",
        "1.2 Lithosphere and asthenosphere",
        "2 Plates in motion",
        "2.1 What a plate is",
        "2.1.1 How plates are mapped",
        "2.1.2 Plate thickness",
        "2.1.3 Microplates",
        "2.2 What drives the plates",
        "3 Where plates meet",
        "3.1 Kinds of boundary",
        "3.2 Subduction zones",
    ] {
        assert!(
            rendered.lines().any(|candidate| candidate.trim() == line),
            "{line} in\n{rendered}"
        );
    }
}

#[test]
fn the_profile_rendering_names_each_rung_its_state_the_misconceptions_and_the_goal() {
    let course = sample_course();

    let rendered = render_profile(&course.ladder, &course.profile);

    for line in [
        "- rung 1, Earth's layers: known",
        "- rung 2, Lithosphere and asthenosphere: known",
        "- rung 3, Tectonic plates: shaky",
        "- rung 4, Mantle convection: unknown",
        "- rung 8, Earthquakes and volcanoes at boundaries: unknown",
        "- rung 4: The mantle is a sea of liquid rock.",
    ] {
        assert!(rendered.contains(line), "{line} in\n{rendered}");
    }
    assert!(rendered.contains("working knowledge"), "{rendered}");
}

#[test]
fn a_section_relying_on_a_concept_four_chapters_back_gets_its_whole_course_memory() {
    let memory = long_memory();

    let context = context(&memory, MOUNTAIN_BELTS);
    let input = context.input();

    // The parts, in order.
    let kinds: Vec<PartKind> = context.parts.iter().map(|part| part.kind).collect();
    assert_eq!(
        kinds,
        [
            PartKind::Profile,
            PartKind::Index,
            PartKind::Conventions,
            PartKind::Entry,
            PartKind::Prerequisites,
            PartKind::Previous,
            PartKind::Promises,
            PartKind::OtherRead,
        ]
    );
    // The introducing section's record, four chapters back.
    let prerequisites = part(&context, PartKind::Prerequisites);
    assert!(prerequisites.contains(LAYERS_SUMMARY), "{prerequisites}");
    assert!(
        prerequisites.contains("The thin rocky skin we live on."),
        "{prerequisites}"
    );
    // The previous section's record.
    let previous = part(&context, PartKind::Previous);
    assert!(previous.contains(RIDGES_SUMMARY), "{previous}");
    assert!(previous.contains(RIDGE_LINE), "{previous}");
    // The conventions.
    let conventions = part(&context, PartKind::Conventions);
    for convention in &memory.conventions {
        assert!(conventions.contains(&convention.key), "{conventions}");
        assert!(conventions.contains(&convention.meaning), "{conventions}");
    }
    // Its own entry.
    let entry = part(&context, PartKind::Entry);
    assert!(entry.contains("Mountain belts"), "{entry}");
    assert!(entry.contains("mountain belt"), "{entry}");
    assert!(entry.contains("crust"), "{entry}");
    // Every rung with its state.
    let profile = part(&context, PartKind::Profile);
    for (rung, state) in [
        ("rung 1, Earth's layers", "known"),
        ("rung 2, Lithosphere and asthenosphere", "known"),
        ("rung 3, Tectonic plates", "shaky"),
        ("rung 4, Mantle convection", "unknown"),
        ("rung 5, Plate boundaries", "unknown"),
        ("rung 6, Seafloor spreading", "unknown"),
        ("rung 7, Subduction", "unknown"),
        ("rung 8, Earthquakes and volcanoes at boundaries", "unknown"),
    ] {
        assert!(profile.contains(&format!("{rung}: {state}")), "{profile}");
    }
    // Every index title.
    let index = part(&context, PartKind::Index);
    for chapter in &memory.index.chapters {
        assert!(index.contains(&chapter.title), "{index}");
    }
    for entry in in_index_order(&memory.index) {
        assert!(index.contains(&entry.section.title), "{index}");
    }
    // One line for each other section read, under its chapter.
    let other = part(&context, PartKind::OtherRead);
    let lines: Vec<&str> = other.lines().map(str::trim).collect();
    let under = |chapter: &str, line: &str| {
        let chapter_at = lines.iter().position(|candidate| *candidate == chapter);
        let line_at = lines.iter().position(|candidate| *candidate == line);
        match (chapter_at, line_at) {
            (Some(chapter_at), Some(line_at)) => {
                chapter_at < line_at
                    && lines[chapter_at + 1..line_at]
                        .iter()
                        .all(|between| between.starts_with("- "))
            }
            _ => false,
        }
    };
    for (chapter, line) in [
        (
            "1 Earth's structure",
            "- 1.2 Lithosphere and asthenosphere: The rigid shell and the soft layer it rides on.",
        ),
        (
            "2 Plates in motion",
            "- 2.1 What a plate is: A rigid piece of the lithosphere that moves as one.",
        ),
        (
            "2 Plates in motion",
            "- 2.1.1 How plates are mapped: Earthquake belts and satellite positions draw the edges.",
        ),
        (
            "3 Where plates meet",
            "- 3.1 Kinds of boundary: Plates pull apart, collide or slide past each other.",
        ),
    ] {
        assert!(under(chapter, line), "{line} under {chapter} in\n{other}");
    }
    assert_eq!(
        lines.iter().filter(|line| line.starts_with("- ")).count(),
        4,
        "{other}"
    );
    // Sections whose record is sent, and unread ones, have no line there.
    for absent in ["Layers of the Earth", "Mid-ocean ridges", "Plate thickness"] {
        assert!(!other.contains(absent), "{absent} in\n{other}");
    }
    assert!(!input.contains(KINDS_SUMMARY), "{input}");
    assert!(!input.contains(LITHOSPHERE_SUMMARY), "{input}");
}

#[test]
fn a_promise_to_this_section_appears_and_one_to_another_section_does_not() {
    // "What drives the plates" (2.2) relies on concepts from 1.1, 1.2 and
    // 2.1, whose records promise 1.2, 2.1 and 2.2 something.
    let context = context(&sample_memory(), WHAT_DRIVES_PLATES);
    let input = context.input();

    let promises = part(&context, PartKind::Promises);
    assert!(
        promises.contains("what makes the plates move"),
        "{promises}"
    );
    assert!(promises.contains("What a plate is"), "{promises}");
    assert!(!input.contains("what a single plate is"), "{input}");
    assert!(
        !input.contains("how the outer layers split by strength"),
        "{input}"
    );
}

#[test]
fn a_deep_dive_gets_its_parents_record() {
    let context = context(&sample_memory(), PLATE_THICKNESS);

    let parent = part(&context, PartKind::Parent);
    assert!(parent.contains("What a plate is"), "{parent}");
    assert!(parent.contains(WHAT_A_PLATE_IS_SUMMARY), "{parent}");
    assert!(
        parent.contains("the Atlantic widening by about 2.5 cm a year"),
        "{parent}"
    );
}

#[test]
fn an_unconfirmed_record_is_sent_as_it_is() {
    // The section before "Plate thickness" is "How plates are mapped", whose
    // record was predicted from its map entry.
    let context = context(&sample_memory(), PLATE_THICKNESS);

    let previous = part(&context, PartKind::Previous);
    assert!(previous.contains("How plates are mapped"), "{previous}");
    assert!(previous.contains("plate velocity"), "{previous}");
    assert!(previous.contains("unconfirmed"), "{previous}");
}

#[test]
fn a_prerequisite_not_yet_read_is_named_and_no_record_is_sent_for_it() {
    let mut memory = sample_memory();
    set_read(&mut memory, WHAT_A_PLATE_IS, false);
    entry_mut(&mut memory, WHAT_DRIVES_PLATES)
        .relies_on
        .push("plate flexure".to_owned());

    let context = context(&memory, WHAT_DRIVES_PLATES);
    let input = context.input();

    let prerequisites = part(&context, PartKind::Prerequisites);
    for concept in ["tectonic plate", "plate flexure"] {
        assert!(
            prerequisites
                .lines()
                .any(|line| line.contains(concept) && line.contains("not yet read")),
            "{concept} not yet read in\n{prerequisites}"
        );
    }
    assert!(!input.contains(WHAT_A_PLATE_IS_SUMMARY), "{input}");
    assert!(
        !input.contains("the Atlantic widening by about 2.5 cm a year"),
        "{input}"
    );
    // The read introducers still send their records.
    assert!(prerequisites.contains(LAYERS_SUMMARY), "{prerequisites}");
    assert!(
        prerequisites.contains(LITHOSPHERE_SUMMARY),
        "{prerequisites}"
    );
}

#[test]
fn the_introducers_new_current_version_is_the_record_sent() {
    let mut memory = sample_memory();
    let new_summary = "Rewritten: three shells, told from the core outwards.";
    memory.records.insert(
        LAYERS,
        confirmed(SectionRecord {
            summary: Some(new_summary.to_owned()),
            concepts: vec![
                taught("crust", "The outermost shell, thinnest of all."),
                taught("mantle", "The thick shell of slowly moving rock."),
                taught("core", "The metal heart of the planet."),
            ],
            notation: Vec::new(),
            terms: Vec::new(),
            examples: Vec::new(),
            promises: Vec::new(),
        }),
    );

    let input = context(&memory, WHAT_DRIVES_PLATES).input();

    assert!(input.contains(new_summary), "{input}");
    assert!(
        input.contains("The thick shell of slowly moving rock."),
        "{input}"
    );
    assert!(!input.contains(LAYERS_SUMMARY), "{input}");
    assert!(!input.contains("Hot rock below the crust"), "{input}");
}

#[test]
fn a_section_without_a_map_entry_is_written_with_an_empty_one() {
    let mut memory = sample_memory();
    memory
        .map
        .entries
        .retain(|entry| entry.section != WHAT_DRIVES_PLATES);

    let context = context(&memory, WHAT_DRIVES_PLATES);

    let entry = part(&context, PartKind::Entry);
    assert!(entry.contains("What drives the plates"), "{entry}");
    assert!(!entry.contains("mantle convection"), "{entry}");
    assert!(
        context.part(PartKind::Prerequisites).is_none(),
        "{context:#?}"
    );
}

#[test]
fn no_request_holds_another_sections_text_or_a_side_chat() {
    const BLOCK_SENTINEL: &str = "SENTINEL-BLOCK-41c9";
    const CHAT_SENTINEL: &str = "SENTINEL-CHAT-41c9";
    let memory = long_memory();
    // Block text and side chats live beside the course memory, in the store.
    let mut blocks: HashMap<Uuid, String> = HashMap::new();
    let mut chats: HashMap<Uuid, String> = HashMap::new();
    for entry in in_index_order(&memory.index) {
        blocks.insert(
            entry.section.id,
            format!("{} {BLOCK_SENTINEL}", entry.section.title),
        );
        chats.insert(
            entry.section.id,
            format!("why {}? {CHAT_SENTINEL}", entry.section.title),
        );
    }
    // The memory has no place for them: listing every field without `..`
    // stops compiling when one is added.
    let CourseMemory {
        ladder: _,
        profile: _,
        index: _,
        map: _,
        conventions: _,
        records: _,
    } = &memory;

    for entry in in_index_order(&memory.index) {
        let input = context(&memory, entry.section.id).input();
        assert!(!input.is_empty());
        assert!(!input.contains(BLOCK_SENTINEL), "{input}");
        assert!(!input.contains(CHAT_SENTINEL), "{input}");
    }
    assert_eq!(blocks.len(), chats.len());
}

#[test]
fn over_budget_the_context_shrinks_in_the_fixed_order() {
    let memory = long_memory();
    let at = |budget: usize| {
        section_context(&memory, MOUNTAIN_BELTS, Budget(budget)).expect("a section of the index")
    };

    let whole = at(usize::MAX);
    let s0 = size(&whole);
    assert_eq!(at(s0).shrunk, []);
    let titles = at(s0 - 1);
    assert_eq!(titles.shrunk, [Shrink::ChapterTitles]);
    let s1 = size(&titles);
    assert!(s1 < s0);
    let summary = at(s1 - 1);
    assert_eq!(
        summary.shrunk,
        [Shrink::ChapterTitles, Shrink::PreviousSummary]
    );
    let s2 = size(&summary);
    assert!(s2 < s1);
    let lines = at(s2 - 1);
    let all = [
        Shrink::ChapterTitles,
        Shrink::PreviousSummary,
        Shrink::PrerequisiteLines,
    ];
    assert_eq!(lines.shrunk, all);
    let s3 = size(&lines);
    assert!(s3 < s2);

    // Budgets between the sizes apply exactly the steps needed.
    assert_eq!(at(s0 + 10).shrunk, []);
    assert_eq!(at((s0 + s1) / 2).shrunk, [Shrink::ChapterTitles]);
    assert_eq!(at((s1 + s2) / 2).shrunk, all[..2]);
    assert_eq!(at((s2 + s3) / 2).shrunk, all);
    assert_eq!(at(s3 / 2).shrunk, all);
    assert_eq!(at(s3 / 2), lines);

    // The chapter lines became chapter titles.
    let other = part(&titles, PartKind::OtherRead);
    assert!(other.contains("Earth's structure"), "{other}");
    assert!(other.contains("Where plates meet"), "{other}");
    assert!(
        !other.contains("The rigid shell and the soft layer it rides on."),
        "{other}"
    );
    assert!(!other.contains("Lithosphere and asthenosphere"), "{other}");
    // The previous record became its summary.
    let previous = part(&summary, PartKind::Previous);
    assert!(previous.contains(RIDGES_SUMMARY), "{previous}");
    assert!(!previous.contains(RIDGE_LINE), "{previous}");
    assert!(!previous.contains("Iceland"), "{previous}");
    // The prerequisite record became the lines of the concepts relied on.
    let prerequisites = part(&lines, PartKind::Prerequisites);
    assert!(
        prerequisites.contains("The thin rocky skin we live on."),
        "{prerequisites}"
    );
    assert!(!prerequisites.contains(LAYERS_SUMMARY), "{prerequisites}");
    assert!(
        !prerequisites.contains("Hot rock below the crust"),
        "{prerequisites}"
    );
    assert!(!prerequisites.contains("a peach"), "{prerequisites}");

    // Profile, conventions and map entry never change.
    for kind in [PartKind::Profile, PartKind::Conventions, PartKind::Entry] {
        let unshrunk = part(&whole, kind);
        for step in [&titles, &summary, &lines] {
            assert_eq!(part(step, kind), unshrunk, "{kind:?}");
        }
    }
}

/// A text of exactly `chars` characters, not only Latin letters.
fn text(seed: &str, chars: usize) -> String {
    format!("{seed} λόγος — ")
        .chars()
        .cycle()
        .take(chars)
        .collect()
}

/// A confirmed record at the validated limits, up to the record's whole size.
fn record_at_limits(seed: usize, promised: &str) -> StoredRecord {
    let mut record = SectionRecord {
        summary: Some(text(&format!("summary {seed}"), RECORD_SUMMARY_CHARS)),
        concepts: vec![taught(
            &format!("concept {seed}"),
            &text(&format!("line {seed}"), RECORD_LINE_CHARS),
        )],
        notation: Vec::new(),
        terms: Vec::new(),
        examples: Vec::new(),
        promises: vec![promise(
            promised,
            &text(&format!("promise {seed}"), RECORD_LINE_CHARS),
        )],
    };
    let fits = |record: &SectionRecord| {
        serde_json::to_string(record)
            .expect("a record serialises")
            .chars()
            .count()
            <= RECORD_CHARS
    };
    for at in 0..RECORD_ENTRIES {
        let mut grown = record.clone();
        let entry = FixedEntry {
            key: format!("k{seed}.{at}"),
            meaning: text(&format!("meaning {seed}.{at}"), RECORD_LINE_CHARS),
        };
        match at % 3 {
            0 => grown.terms.push(entry),
            1 => grown.notation.push(entry),
            _ => grown
                .examples
                .push(text(&format!("example {seed}.{at}"), RECORD_LINE_CHARS)),
        }
        if !fits(&grown) {
            break;
        }
        record = grown;
    }
    assert!(fits(&record));
    confirmed(record)
}

/// A course of `read` sections, eight to a chapter, every one read, each
/// introducing its own concept and relying on the first, the one before it
/// and the one before that, with records at the validated limits; then a
/// last section relying on the same, and a deep dive under the middle section
/// relying on its parent's concept too, both unread.
fn grown_memory(read: usize) -> (CourseMemory, Uuid, Uuid) {
    let course = sample_course();
    let id = |n: usize| Uuid::from_u128(0x10_0000 + n as u128);
    let mut memory = CourseMemory {
        ladder: course.ladder,
        profile: course.profile,
        index: IndexTree {
            chapters: Vec::new(),
        },
        map: course.map,
        conventions: course.conventions,
        records: HashMap::new(),
    };
    memory.map.concepts.clear();
    memory.map.entries.clear();
    let total = read + 1;
    let parent = read / 2;
    let deep_dive = Uuid::from_u128(0x20_0000);
    for n in 0..total {
        if n % 8 == 0 {
            memory.index.chapters.push(TreeChapter {
                id: Uuid::from_u128(0x30_0000 + n as u128),
                title: text(&format!("Chapter {}", n / 8 + 1), 60),
                sections: Vec::new(),
            });
        }
        let mut node = section(
            id(n),
            &text(&format!("Section {n}"), 60),
            &text(&format!("What section {n} covers"), 120),
            n < read,
        );
        if n == parent {
            node.children = (0..3)
                .map(|child| {
                    section(
                        if child == 0 {
                            deep_dive
                        } else {
                            Uuid::from_u128(0x20_0000 + child)
                        },
                        &text(&format!("Deep dive {child}"), 60),
                        &text("Further", 120),
                        false,
                    )
                })
                .collect();
        }
        let chapter = memory.index.chapters.last_mut().expect("a chapter");
        let place = chapter.sections.len() + 1;
        chapter.sections.push(node);
        memory.map.concepts.push(Concept {
            name: format!("concept {n}"),
            rung: None,
        });
        let mut relies_on = names(&["concept 0"]);
        relies_on.extend((n.saturating_sub(2)..n).map(|before| format!("concept {before}")));
        relies_on.dedup();
        if n == 0 {
            relies_on.clear();
        }
        memory.map.entries.push(MapEntry {
            section: id(n),
            introduces: vec![format!("concept {n}")],
            relies_on: relies_on.clone(),
        });
        if n == parent {
            let mut relies_on = relies_on;
            relies_on.push(format!("concept {n}"));
            relies_on.dedup();
            memory.map.entries.push(MapEntry {
                section: deep_dive,
                introduces: names(&["deep concept"]),
                relies_on,
            });
        }
        if n < read {
            let promised = format!("{}.{}", memory.index.chapters.len(), place + 1);
            memory.records.insert(id(n), record_at_limits(n, &promised));
        }
    }
    (memory, id(read), deep_dive)
}

#[test]
fn the_request_stays_under_the_default_budget_as_the_course_grows() {
    for read in [1, 2, 5, 9, 17, 40, 80, 160] {
        let (memory, last, deep_dive) = grown_memory(read);
        for node in [last, deep_dive] {
            let context =
                section_context(&memory, node, Budget::SECTION).expect("a section of the index");
            assert!(
                size(&context) <= Budget::SECTION.0,
                "{read} read sections: {} characters, {:?}",
                size(&context),
                context.shrunk
            );
        }
    }
}

#[test]
fn a_node_outside_the_index_has_no_context() {
    let memory = sample_memory();

    assert_eq!(
        section_context(&memory, Uuid::from_u128(0xdead), Budget::SECTION),
        None
    );
    assert_eq!(
        section_context(&memory, CHAPTER_STRUCTURE, Budget::SECTION),
        None
    );
}
