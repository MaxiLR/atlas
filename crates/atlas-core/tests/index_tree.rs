//! The index tree's navigation rules on trees built here: Next, index order,
//! reading states, counts, and inserting a deep dive.

use std::collections::HashSet;

use atlas_core::index_tree::{
    IndexTree, ReadingState, TreeChapter, TreeError, TreeSection, first_section, first_section_of,
    furthest_read, in_index_order, insert_children, mark_read, next, progress, state,
};
use atlas_core::protocol::{NodeMark, Progress, SectionKind};
use uuid::Uuid;

const CH_A: Uuid = Uuid::from_u128(0xA00);
const A1: Uuid = Uuid::from_u128(0xA01);
const A2: Uuid = Uuid::from_u128(0xA02);
const CH_B: Uuid = Uuid::from_u128(0xB00);
const B1: Uuid = Uuid::from_u128(0xB01);
/// The last section of chapter B, with a deep dive of three children.
const B2: Uuid = Uuid::from_u128(0xB02);
const D1: Uuid = Uuid::from_u128(0xB21);
/// The middle child, itself deepened with three children.
const D2: Uuid = Uuid::from_u128(0xB22);
const N1: Uuid = Uuid::from_u128(0xB221);
const N2: Uuid = Uuid::from_u128(0xB222);
const N3: Uuid = Uuid::from_u128(0xB223);
const D3: Uuid = Uuid::from_u128(0xB23);
const CH_C: Uuid = Uuid::from_u128(0xC00);
const C1: Uuid = Uuid::from_u128(0xC01);
const C2: Uuid = Uuid::from_u128(0xC02);
const E1: Uuid = Uuid::from_u128(0xC11);
const E2: Uuid = Uuid::from_u128(0xC12);
const E3: Uuid = Uuid::from_u128(0xC13);
const E4: Uuid = Uuid::from_u128(0xC14);
const UNKNOWN: Uuid = Uuid::from_u128(0xFFFF);

const CHAPTERS: [Uuid; 3] = [CH_A, CH_B, CH_C];

fn leaf(id: Uuid, read: bool) -> TreeSection {
    TreeSection {
        id,
        title: format!("Section {id}"),
        line: format!("What {id} covers."),
        read,
        children: Vec::new(),
    }
}

fn branch(id: Uuid, read: bool, children: Vec<TreeSection>) -> TreeSection {
    TreeSection {
        children,
        ..leaf(id, read)
    }
}

fn chapter(id: Uuid, sections: Vec<TreeSection>) -> TreeChapter {
    TreeChapter {
        id,
        title: format!("Chapter {id}"),
        sections,
    }
}

/// Three chapters; B2, the last section of chapter B, has a deep dive of
/// three children, and the middle one has a nested deep dive of three.
/// Read: A1, B1, B2, D1, N1.
fn sample() -> IndexTree {
    IndexTree {
        chapters: vec![
            chapter(CH_A, vec![leaf(A1, true), leaf(A2, false)]),
            chapter(
                CH_B,
                vec![
                    leaf(B1, true),
                    branch(
                        B2,
                        true,
                        vec![
                            leaf(D1, true),
                            branch(
                                D2,
                                false,
                                vec![leaf(N1, true), leaf(N2, false), leaf(N3, false)],
                            ),
                            leaf(D3, false),
                        ],
                    ),
                ],
            ),
            chapter(CH_C, vec![leaf(C1, false), leaf(C2, false)]),
        ],
    }
}

fn all_nodes() -> Vec<Uuid> {
    vec![
        CH_A, A1, A2, CH_B, B1, B2, D1, D2, N1, N2, N3, D3, CH_C, C1, C2,
    ]
}

#[test]
fn next_from_every_node_of_the_tree() {
    let tree = sample();
    let expected = [
        // A chapter opens its first section.
        (CH_A, Some(A1)),
        (A1, Some(A2)),
        // The broad pass crosses chapters.
        (A2, Some(B1)),
        (CH_B, Some(B1)),
        (B1, Some(B2)),
        // From a section with a deep dive, Next stays on the broad pass.
        (B2, Some(C1)),
        // Inside a deep dive, the next sibling.
        (D1, Some(D2)),
        (D2, Some(D3)),
        (N1, Some(N2)),
        (N2, Some(N3)),
        // After the last sibling at depth two: the broad-pass section after
        // the root B2, not D3.
        (N3, Some(C1)),
        // After the last sibling at depth one: the same.
        (D3, Some(C1)),
        (CH_C, Some(C1)),
        (C1, Some(C2)),
        // The end of the course.
        (C2, None),
    ];
    for (node, want) in expected {
        assert_eq!(next(&tree, node), want, "next from {node}");
    }
    assert_eq!(next(&tree, UNKNOWN), None);
}

#[test]
fn next_never_lands_on_a_chapter() {
    let tree = sample();
    for node in all_nodes() {
        if let Some(target) = next(&tree, node) {
            assert!(!CHAPTERS.contains(&target), "next from {node} is {target}");
        }
    }
}

fn order(tree: &IndexTree) -> Vec<(Uuid, u32, SectionKind, Uuid)> {
    in_index_order(tree)
        .iter()
        .map(|entry| (entry.section.id, entry.depth, entry.kind, entry.chapter))
        .collect()
}

#[test]
fn index_order_puts_deep_dives_directly_under_their_section() {
    use SectionKind::{BroadPass as Broad, DeepDive as Deep};
    assert_eq!(
        order(&sample()),
        vec![
            (A1, 0, Broad, CH_A),
            (A2, 0, Broad, CH_A),
            (B1, 0, Broad, CH_B),
            (B2, 0, Broad, CH_B),
            (D1, 1, Deep, CH_B),
            (D2, 1, Deep, CH_B),
            (N1, 2, Deep, CH_B),
            (N2, 2, Deep, CH_B),
            (N3, 2, Deep, CH_B),
            (D3, 1, Deep, CH_B),
            (C1, 0, Broad, CH_C),
            (C2, 0, Broad, CH_C),
        ]
    );
}

fn reading(mark: NodeMark, read: bool) -> Option<ReadingState> {
    Some(ReadingState { mark, read })
}

#[test]
fn marks_counts_and_the_furthest_read_section_follow_reading() {
    use NodeMark::{Branch, Read, Unread};
    let mut tree = sample();

    let before = [
        (A1, reading(Read, true)),
        (A2, reading(Unread, false)),
        (B1, reading(Read, true)),
        // A section with deep dives shows the branch, read or not.
        (B2, reading(Branch, true)),
        (D1, reading(Read, true)),
        (D2, reading(Branch, false)),
        (N1, reading(Read, true)),
        (N2, reading(Unread, false)),
        (N3, reading(Unread, false)),
        (D3, reading(Unread, false)),
        (C1, reading(Unread, false)),
        (C2, reading(Unread, false)),
        // Chapters are stations, not sections.
        (CH_A, None),
        (UNKNOWN, None),
    ];
    for (node, want) in before {
        assert_eq!(state(&tree, node), want, "state of {node}");
    }
    // Six broad-pass sections and eight deep dives present.
    assert_eq!(progress(&tree), Progress { read: 5, total: 14 });
    // A2 is unread, but the line is bright up to the furthest read B2.
    assert_eq!(furthest_read(&tree), Some(B2));
    assert_eq!(first_section(&tree), Some(A1));
    assert_eq!(first_section_of(&tree, CH_A), Some(A1));
    assert_eq!(first_section_of(&tree, CH_B), Some(B1));
    assert_eq!(first_section_of(&tree, CH_C), Some(C1));
    assert_eq!(first_section_of(&tree, B1), None);
    assert_eq!(first_section_of(&tree, UNKNOWN), None);

    assert!(mark_read(&mut tree, C1));
    assert!(mark_read(&mut tree, N3));
    assert!(mark_read(&mut tree, D2));
    // Marking a read section again changes nothing.
    assert!(mark_read(&mut tree, A1));
    assert!(!mark_read(&mut tree, CH_A));
    assert!(!mark_read(&mut tree, UNKNOWN));

    assert_eq!(state(&tree, C1), reading(Read, true));
    assert_eq!(state(&tree, N3), reading(Read, true));
    assert_eq!(state(&tree, D2), reading(Branch, true));
    assert_eq!(state(&tree, A2), reading(Unread, false));
    assert_eq!(progress(&tree), Progress { read: 8, total: 14 });
    assert_eq!(furthest_read(&tree), Some(C1));
    // A deep dive read does not move the bright part of the line.
    assert!(mark_read(&mut tree, D3));
    assert_eq!(furthest_read(&tree), Some(C1));
}

#[test]
fn an_unread_course_starts_dark() {
    let mut tree = sample();
    for chapter in &mut tree.chapters {
        fn clear(sections: &mut [TreeSection]) {
            for section in sections {
                section.read = false;
                clear(&mut section.children);
            }
        }
        clear(&mut chapter.sections);
    }
    assert_eq!(furthest_read(&tree), None);
    assert_eq!(progress(&tree), Progress { read: 0, total: 14 });
    assert_eq!(first_section(&IndexTree { chapters: vec![] }), None);
}

fn deep_dive() -> Vec<TreeSection> {
    vec![
        leaf(E1, false),
        leaf(E2, false),
        leaf(E3, false),
        leaf(E4, false),
    ]
}

#[test]
fn inserting_children_keeps_their_order_and_the_rest_of_the_tree() {
    use NodeMark::{Branch, Unread};
    let mut tree = sample();
    mark_read(&mut tree, C1);

    insert_children(&mut tree, C1, deep_dive()).unwrap();

    let mut expected = sample();
    expected.chapters[2].sections[0] = branch(C1, true, deep_dive());
    assert_eq!(tree, expected);

    let entries = order(&tree);
    let ids: Vec<Uuid> = entries.iter().map(|entry| entry.0).collect();
    assert_eq!(
        ids,
        vec![
            A1, A2, B1, B2, D1, D2, N1, N2, N3, D3, C1, E1, E2, E3, E4, C2
        ]
    );
    assert_eq!(entries[11], (E1, 1, SectionKind::DeepDive, CH_C));

    assert_eq!(state(&tree, C1), reading(Branch, true));
    assert_eq!(state(&tree, E1), reading(Unread, false));
    assert_eq!(progress(&tree), Progress { read: 6, total: 18 });
    assert_eq!(furthest_read(&tree), Some(C1));

    assert_eq!(next(&tree, C1), Some(C2));
    assert_eq!(next(&tree, E1), Some(E2));
    assert_eq!(next(&tree, E3), Some(E4));
    assert_eq!(next(&tree, E4), Some(C2));
}

#[test]
fn a_nested_deep_dive_can_be_inserted_under_a_deep_dive() {
    let mut tree = sample();
    insert_children(&mut tree, N2, deep_dive()[..3].to_vec()).unwrap();
    assert_eq!(state(&tree, N2).unwrap().mark, NodeMark::Branch);
    assert_eq!(
        order(&tree)[7..11]
            .iter()
            .map(|entry| (entry.0, entry.1))
            .collect::<Vec<_>>(),
        vec![(N2, 2), (E1, 3), (E2, 3), (E3, 3)]
    );
    // Back to the broad pass after the root B2, from depth three.
    assert_eq!(next(&tree, E3), Some(C1));
}

#[test]
fn inserting_children_refuses_and_changes_nothing() {
    let mut tree = sample();
    let refusals = [
        (C1, deep_dive()[..2].to_vec(), TreeError::ChildCount(2)),
        (
            C1,
            (0xC11..0xC18)
                .map(|id| leaf(Uuid::from_u128(id), false))
                .collect(),
            TreeError::ChildCount(7),
        ),
        (UNKNOWN, deep_dive(), TreeError::UnknownSection(UNKNOWN)),
        (CH_A, deep_dive(), TreeError::UnknownSection(CH_A)),
        (
            C1,
            vec![leaf(E1, false), leaf(A2, false), leaf(E3, false)],
            TreeError::DuplicateId(A2),
        ),
        (
            C1,
            vec![leaf(E1, false), leaf(E2, false), leaf(E1, false)],
            TreeError::DuplicateId(E1),
        ),
    ];
    for (parent, children, error) in refusals {
        assert_eq!(insert_children(&mut tree, parent, children), Err(error));
        assert_eq!(tree, sample());
    }
}

// The sample holds six broad-pass sections (A1, A2, B1, B2, C1, C2) and six
// deep dives (D1 to D3, N1 to N3): twelve sections present, sixteen once C1
// is deepened with four.
#[test]
fn counts_cover_the_broad_pass_and_every_deep_dive_present() {
    use NodeMark::{Branch, Read, Unread};
    let mut tree = sample();
    assert_eq!(progress(&tree), Progress { read: 5, total: 12 });
    assert_eq!(furthest_read(&tree), Some(B2));
    assert_eq!(first_section(&tree), Some(A1));
    assert_eq!(first_section_of(&tree, CH_A), Some(A1));
    assert_eq!(first_section_of(&tree, CH_B), Some(B1));
    assert_eq!(first_section_of(&tree, CH_C), Some(C1));
    assert_eq!(first_section_of(&tree, B1), None);
    assert_eq!(first_section_of(&tree, UNKNOWN), None);

    assert!(mark_read(&mut tree, C1));
    assert!(mark_read(&mut tree, N3));
    assert!(mark_read(&mut tree, D2));
    assert!(mark_read(&mut tree, A1));
    assert!(!mark_read(&mut tree, CH_A));
    assert!(!mark_read(&mut tree, UNKNOWN));
    assert_eq!(state(&tree, C1), reading(Read, true));
    assert_eq!(state(&tree, N3), reading(Read, true));
    assert_eq!(state(&tree, D2), reading(Branch, true));
    assert_eq!(state(&tree, A2), reading(Unread, false));
    assert_eq!(progress(&tree), Progress { read: 8, total: 12 });
    assert_eq!(furthest_read(&tree), Some(C1));
    assert!(mark_read(&mut tree, D3));
    assert_eq!(furthest_read(&tree), Some(C1));

    insert_children(&mut tree, C1, deep_dive()).unwrap();
    assert_eq!(state(&tree, C1), reading(Branch, true));
    assert_eq!(state(&tree, E1), reading(Unread, false));
    assert_eq!(progress(&tree), Progress { read: 9, total: 16 });
    assert_eq!(furthest_read(&tree), Some(C1));
    assert_eq!(next(&tree, C1), Some(C2));
    assert_eq!(next(&tree, E1), Some(E2));
    assert_eq!(next(&tree, E3), Some(E4));
    assert_eq!(next(&tree, E4), Some(C2));
}

#[test]
fn an_unread_course_counts_nothing_read() {
    let mut tree = sample();
    for chapter in &mut tree.chapters {
        fn clear(sections: &mut [TreeSection]) {
            for section in sections {
                section.read = false;
                clear(&mut section.children);
            }
        }
        clear(&mut chapter.sections);
    }
    assert_eq!(furthest_read(&tree), None);
    assert_eq!(progress(&tree), Progress { read: 0, total: 12 });
    assert_eq!(first_section(&IndexTree { chapters: vec![] }), None);
}

/// A small deterministic generator, so the property runs the same each time.
struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: u64) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) % bound
    }
}

struct Generated {
    tree: IndexTree,
    broad_pass: Vec<Uuid>,
    deep_dives: HashSet<Uuid>,
    chapters: HashSet<Uuid>,
}

fn generate(rng: &mut Rng) -> Generated {
    let mut ids = 1..;
    let mut generated = Generated {
        tree: IndexTree {
            chapters: Vec::new(),
        },
        broad_pass: Vec::new(),
        deep_dives: HashSet::new(),
        chapters: HashSet::new(),
    };
    fn deepen(
        rng: &mut Rng,
        ids: &mut std::ops::RangeFrom<u128>,
        depth: u32,
        deep_dives: &mut HashSet<Uuid>,
    ) -> Vec<TreeSection> {
        if depth > 3 || rng.below(3) != 0 {
            return Vec::new();
        }
        (0..3 + rng.below(4))
            .map(|_| {
                let id = Uuid::from_u128(ids.next().unwrap());
                deep_dives.insert(id);
                branch(
                    id,
                    rng.below(2) == 0,
                    deepen(rng, ids, depth + 1, deep_dives),
                )
            })
            .collect()
    }
    for _ in 0..1 + rng.below(5) {
        let chapter_id = Uuid::from_u128(ids.next().unwrap());
        generated.chapters.insert(chapter_id);
        // A chapter may be empty, so Next has to cross it.
        let sections = (0..rng.below(5))
            .map(|_| {
                let id = Uuid::from_u128(ids.next().unwrap());
                generated.broad_pass.push(id);
                let children = deepen(rng, &mut ids, 1, &mut generated.deep_dives);
                branch(id, rng.below(2) == 0, children)
            })
            .collect();
        generated.tree.chapters.push(chapter(chapter_id, sections));
    }
    generated
}

#[test]
fn following_next_from_the_first_section_walks_the_broad_pass_once() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..500 {
        let generated = generate(&mut rng);
        let tree = &generated.tree;

        let mut visited = Vec::new();
        let mut at = first_section(tree);
        while let Some(node) = at {
            assert!(
                visited.len() <= generated.broad_pass.len(),
                "Next loops: {visited:?}"
            );
            assert!(!generated.deep_dives.contains(&node), "visited deep dive");
            visited.push(node);
            at = next(tree, node);
        }
        assert_eq!(visited, generated.broad_pass);

        let every_node = generated
            .chapters
            .iter()
            .chain(&generated.broad_pass)
            .chain(&generated.deep_dives);
        for &node in every_node {
            if let Some(target) = next(tree, node) {
                assert!(!generated.chapters.contains(&target), "{node} -> chapter");
            }
        }
    }
}
