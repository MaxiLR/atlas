//! Go deeper through the core's request handler: the propose-deep-dives task
//! answered by a scripted provider, its children stored under the section
//! with their map entries and new concepts, announced as the tree only, and
//! answered from the store when asked again; the children opened through the
//! sections feature, each with its own entry and its parent's record.
//!
//! The sample course is laid down through the store's repository API on the
//! same database file in a temp dir. Deep dives go under "Lithosphere and
//! asthenosphere" (key 1.2), whose record is confirmed. Before it in the index
//! the course introduces crust, mantle and core (1.1), then lithosphere and
//! asthenosphere (1.2); "tectonic plate" comes later (2.1). Expected maps are
//! written out by hand.

mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::Core;
use atlas_core::domain::conventions::{Convention, ConventionOrigin};
use atlas_core::domain::course_map::{Concept, CourseMap, MapEntry};
use atlas_core::domain::ladder::RungId;
use atlas_core::domain::record::StoredRecord;
use atlas_core::index_tree::{IndexTree, TreeSection};
use atlas_core::protocol::{
    Block, BlockContent, CoreError, CoreEvent, CoreRequest, CoreResponse, NodeMark, SectionKind,
    SectionNode, SectionView, VersionStyle,
};
use atlas_core::provider::{ProviderRequest, Task};
use atlas_core::store::{NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, FakeProvider, FakeScript, RecordingEvents};
use serde_json::{Value, json};
use support::sample_course::{
    HOW_PLATES_ARE_MAPPED, LITHOSPHERE, MICROPLATES, PLATE_THICKNESS, SampleCourse,
    WHAT_A_PLATE_IS, WHAT_DRIVES_PLATES, sample_course,
};
use tempfile::TempDir;
use uuid::Uuid;

/// The start of the confirmed record of "Lithosphere and asthenosphere".
const LITHOSPHERE_SUMMARY: &str = "By strength, the crust and the top of the mantle form one rigid";
/// A phrase only the record of `complete.md` holds.
const WRITTEN_SUMMARY: &str = "sentinel-summary-71c3";

/// The concepts the four proposals introduce; no title or line names them.
const NEW_CONCEPTS: [&str; 4] = [
    "brittle-ductile transition",
    "lithospheric root",
    "isostasy",
    "postglacial rebound",
];

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sections")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

// -- proposals -------------------------------------------------------------------

fn child(
    title: &str,
    summary: &str,
    introduces: &[(&str, Option<u32>)],
    relies_on: &[&str],
) -> Value {
    json!({
        "title": title,
        "summary": summary,
        "introduces": introduces
            .iter()
            .map(|(name, rung)| json!({ "name": name, "rung": rung }))
            .collect::<Vec<_>>(),
        "relies_on": relies_on,
    })
}

fn answer(children: Vec<Value>) -> String {
    json!({ "children": children }).to_string()
}

/// Four children under 1.2, each relying on what 1.1, 1.2 or an earlier
/// sibling introduces.
fn four_children() -> Vec<Value> {
    vec![
        child(
            "Where rock stops breaking",
            "Why rock near the surface snaps and rock below flows.",
            &[("brittle-ductile transition", Some(2))],
            &["lithosphere", "asthenosphere"],
        ),
        child(
            "Roots under mountains",
            "How far down the rigid shell reaches beneath continents.",
            &[("lithospheric root", None)],
            &["lithosphere", "brittle-ductile transition"],
        ),
        child(
            "Floating in balance",
            "Why high ground rides on deep foundations.",
            &[("isostasy", Some(2))],
            &["asthenosphere", "lithospheric root"],
        ),
        child(
            "Land that rises after ice",
            "Scandinavia still lifting since the last ice age.",
            &[("postglacial rebound", None)],
            &["isostasy", "crust"],
        ),
    ]
}

fn four_titles() -> Vec<&'static str> {
    vec![
        "Where rock stops breaking",
        "Roots under mountains",
        "Floating in balance",
        "Land that rises after ice",
    ]
}

fn four_summaries() -> Vec<&'static str> {
    vec![
        "Why rock near the surface snaps and rock below flows.",
        "How far down the rigid shell reaches beneath continents.",
        "Why high ground rides on deep foundations.",
        "Scandinavia still lifting since the last ice age.",
    ]
}

/// The map entries the four children get, in order, by concept name.
fn four_entries() -> Vec<(Vec<&'static str>, Vec<&'static str>)> {
    vec![
        (
            vec!["brittle-ductile transition"],
            vec!["lithosphere", "asthenosphere"],
        ),
        (
            vec!["lithospheric root"],
            vec!["lithosphere", "brittle-ductile transition"],
        ),
        (vec!["isostasy"], vec!["asthenosphere", "lithospheric root"]),
        (vec!["postglacial rebound"], vec!["isostasy", "crust"]),
    ]
}

/// Three grandchildren under the first child, relying on what it introduced.
fn three_grandchildren() -> Vec<Value> {
    vec![
        child(
            "Earthquakes stop at depth",
            "Why quakes fade below a certain depth.",
            &[("seismogenic depth", None)],
            &["brittle-ductile transition"],
        ),
        child(
            "Hot rock, weak rock",
            "How temperature moves the line between snapping and flowing.",
            &[("geotherm", None)],
            &["brittle-ductile transition", "mantle"],
        ),
        child(
            "Water and weakness",
            "How fluids let rock flow at shallower depths.",
            &[("hydrolytic weakening", None)],
            &["geotherm"],
        ),
    ]
}

// -- the harness -----------------------------------------------------------------

fn prose_version(id: Uuid, node: Uuid, record: StoredRecord) -> NewSectionVersion {
    NewSectionVersion {
        id,
        node,
        style: VersionStyle::Original,
        blocks: vec![Block {
            index: 0,
            content: BlockContent::Prose {
                text: "The text of an earlier section.".to_owned(),
            },
        }],
        checks: Vec::new(),
        citations: Vec::new(),
        sources: Vec::new(),
        written_without_search: false,
        record,
        model: Some("gpt-5".to_owned()),
        prompt: "write-section@1".to_owned(),
    }
}

/// Lays the course down: ladder, profile, index with its map and the plan's
/// conventions, and a version for every record.
fn store_course(store: &Store, sample: &SampleCourse) -> Uuid {
    let course = store.create_course(&sample.topic).unwrap().id;
    store.store_ladder(course, &sample.ladder).unwrap();
    store.save_profile(course, &sample.profile).unwrap();
    let fixed_by = |origin: ConventionOrigin| -> Vec<Convention> {
        sample
            .conventions
            .iter()
            .filter(|convention| convention.origin == origin)
            .cloned()
            .collect()
    };
    store
        .insert_index(
            course,
            &sample.index,
            &sample.map,
            &fixed_by(ConventionOrigin::Plan),
        )
        .unwrap();
    for record in &sample.records {
        store
            .insert_version(
                &prose_version(record.version, record.section, record.record.clone()),
                &fixed_by(ConventionOrigin::SectionVersion {
                    version: record.version,
                }),
            )
            .unwrap();
    }
    course
}

struct Harness {
    _dir: TempDir,
    provider: Arc<FakeProvider>,
    events: Arc<RecordingEvents>,
    core: Core,
    store: Store,
    course: Uuid,
}

impl Harness {
    /// The core with the provider `script` sets up, on the sample course.
    fn new(script: impl FnOnce(FakeProvider) -> FakeProvider) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlas.sqlite3");
        let ports = FakePorts::default();
        let provider = Arc::new(script(FakeProvider::new(FakeScript::sample())));
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider.clone(), events.clone()))
            .expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();
        let course = store_course(&store, &sample_course());
        events.take();
        Self {
            _dir: dir,
            provider,
            events,
            core,
            store,
            course,
        }
    }

    /// Proposals answered with these texts, in order: the first answer, then
    /// the repair.
    fn proposing(answers: Vec<String>) -> Self {
        Self::new(|provider| {
            let (first, repairs) = answers.split_first().expect("one answer at least");
            provider
                .answer(Task::ProposeDeepDives, [Ok(first.clone())])
                .answer(Task::RepairStructured, repairs.iter().cloned().map(Ok))
        })
    }

    async fn go_deeper(&self, section_id: Uuid) -> Result<Vec<SectionNode>, CoreError> {
        self.core
            .handle(CoreRequest::GoDeeper {
                course_id: self.course,
                section_id,
            })
            .await
            .map(|response| match response {
                CoreResponse::DeepDives {
                    course_id,
                    section_id: answered,
                    deep_dives,
                } => {
                    assert_eq!(course_id, self.course);
                    assert_eq!(answered, section_id);
                    deep_dives
                }
                other => panic!("expected deep dives, got {other:?}"),
            })
    }

    async fn open(&self, section_id: Uuid) -> Result<SectionView, CoreError> {
        self.core
            .handle(CoreRequest::OpenSection {
                course_id: self.course,
                section_id,
            })
            .await
            .map(|response| match response {
                CoreResponse::Section { section } => section,
                other => panic!("expected a section, got {other:?}"),
            })
    }

    fn requests_for(&self, task: Task) -> Vec<ProviderRequest> {
        self.provider
            .requests()
            .into_iter()
            .filter(|request| request.task == task)
            .collect()
    }

    fn tree(&self) -> IndexTree {
        self.store.index_tree(self.course).unwrap()
    }

    fn map(&self) -> CourseMap {
        self.store.course_map(self.course).unwrap()
    }

    /// The index-changed events emitted since the last call.
    fn index_changes(&self) -> Vec<CoreEvent> {
        self.events
            .take()
            .into_iter()
            .filter(|event| matches!(event, CoreEvent::IndexChanged { .. }))
            .collect()
    }
}

fn find(tree: &IndexTree, id: Uuid) -> TreeSection {
    fn walk(sections: &[TreeSection], id: Uuid) -> Option<&TreeSection> {
        sections.iter().find_map(|section| {
            if section.id == id {
                Some(section)
            } else {
                walk(&section.children, id)
            }
        })
    }
    tree.chapters
        .iter()
        .find_map(|chapter| walk(&chapter.sections, id))
        .cloned()
        .unwrap_or_else(|| panic!("section {id} is in the index"))
}

fn find_node(nodes: &[SectionNode], id: Uuid) -> Option<&SectionNode> {
    nodes.iter().find_map(|node| {
        if node.id == id {
            Some(node)
        } else {
            find_node(&node.deep_dives, id)
        }
    })
}

/// The part of a request's input under the heading starting with `heading`,
/// up to the next top-level heading.
fn part<'a>(input: &'a str, heading: &str) -> &'a str {
    let start = input
        .find(heading)
        .unwrap_or_else(|| panic!("the input has no \"{heading}\":\n{input}"));
    let rest = &input[start..];
    rest[1..]
        .find("\n\n# ")
        .map_or(rest, |end| &rest[..end + 1])
}

fn ids(nodes: &[SectionNode]) -> Vec<Uuid> {
    nodes.iter().map(|node| node.id).collect()
}

fn titles(nodes: &[SectionNode]) -> Vec<&str> {
    nodes.iter().map(|node| node.title.as_str()).collect()
}

fn texts(names: &[&str]) -> Vec<String> {
    names.iter().map(|&name| name.to_owned()).collect()
}

// -- storing the children --------------------------------------------------------

#[tokio::test]
async fn four_proposals_are_stored_as_four_children_in_order_under_the_section() {
    let harness = Harness::proposing(vec![answer(four_children())]);

    let children = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    assert_eq!(titles(&children), four_titles());
    for (node, summary) in children.iter().zip(four_summaries()) {
        assert_eq!(node.summary, summary);
        assert_eq!(node.kind, SectionKind::DeepDive);
        assert!(!node.read);
        assert_eq!(node.mark, NodeMark::Unread);
        assert!(node.deep_dives.is_empty());
    }
    assert_eq!(harness.provider.requests().len(), 1);
    assert_eq!(harness.requests_for(Task::ProposeDeepDives).len(), 1);

    let stored = find(&harness.tree(), LITHOSPHERE);
    let stored_ids: Vec<Uuid> = stored.children.iter().map(|child| child.id).collect();
    assert_eq!(stored_ids, ids(&children));
    let stored_titles: Vec<&str> = stored
        .children
        .iter()
        .map(|child| child.title.as_str())
        .collect();
    assert_eq!(stored_titles, four_titles());
}

#[tokio::test]
async fn the_stored_map_has_an_entry_for_each_child_with_its_concepts() {
    let harness = Harness::proposing(vec![answer(four_children())]);

    let children = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    let map = harness.map();
    for (node, (introduces, relies_on)) in children.iter().zip(four_entries()) {
        let entries: Vec<&MapEntry> = map
            .entries
            .iter()
            .filter(|entry| entry.section == node.id)
            .collect();
        assert_eq!(
            entries,
            [&MapEntry {
                section: node.id,
                introduces: texts(&introduces),
                relies_on: texts(&relies_on),
            }],
            "{}",
            node.title
        );
    }
    let sample = sample_course().map;
    assert_eq!(map.entries.len(), sample.entries.len() + 4);
    assert_eq!(map.concepts[..sample.concepts.len()], sample.concepts[..]);
    assert_eq!(
        map.concepts[sample.concepts.len()..],
        [
            Concept {
                name: "brittle-ductile transition".to_owned(),
                rung: Some(RungId(2)),
            },
            Concept {
                name: "lithospheric root".to_owned(),
                rung: None,
            },
            Concept {
                name: "isostasy".to_owned(),
                rung: Some(RungId(2)),
            },
            Concept {
                name: "postglacial rebound".to_owned(),
                rung: None,
            },
        ]
    );
}

#[tokio::test]
async fn the_index_change_carries_the_tree_with_the_children_and_no_concept_name() {
    let harness = Harness::proposing(vec![answer(four_children())]);

    let children = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    let changes = harness.index_changes();
    assert_eq!(changes.len(), 1, "{changes:?}");
    let CoreEvent::IndexChanged {
        course_id,
        chapters,
    } = &changes[0]
    else {
        unreachable!("filtered to index changes");
    };
    assert_eq!(*course_id, harness.course);
    let sections: Vec<SectionNode> = chapters
        .iter()
        .flat_map(|chapter| chapter.sections.clone())
        .collect();
    let parent = find_node(&sections, LITHOSPHERE).expect("the section is in the tree");
    assert_eq!(parent.mark, NodeMark::Branch);
    assert_eq!(parent.deep_dives, children);
    assert_eq!(titles(&parent.deep_dives), four_titles());

    let json = serde_json::to_string(&changes[0]).unwrap();
    for concept in NEW_CONCEPTS {
        assert!(!json.contains(concept), "{concept} in {json}");
    }
}

#[tokio::test]
async fn next_from_the_last_child_returns_the_broad_pass_section_after_the_root() {
    let harness = Harness::proposing(vec![answer(four_children())]);
    let children = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    let next = harness
        .core
        .handle(CoreRequest::Next {
            course_id: harness.course,
            section_id: children[3].id,
        })
        .await;

    assert_eq!(
        next,
        Ok(CoreResponse::NextSection {
            course_id: harness.course,
            section_id: WHAT_A_PLATE_IS,
        })
    );
    let next = harness
        .core
        .handle(CoreRequest::Next {
            course_id: harness.course,
            section_id: children[1].id,
        })
        .await;
    assert_eq!(
        next,
        Ok(CoreResponse::NextSection {
            course_id: harness.course,
            section_id: children[2].id,
        })
    );
}

#[tokio::test]
async fn a_child_relying_on_a_concept_an_earlier_sibling_introduces_is_accepted() {
    let harness = Harness::proposing(vec![answer(vec![
        child(
            "Where rock stops breaking",
            "Why rock near the surface snaps and rock below flows.",
            &[("brittle-ductile transition", Some(2))],
            &["lithosphere"],
        ),
        child(
            "Roots under mountains",
            "How far down the rigid shell reaches beneath continents.",
            &[("lithospheric root", None)],
            &["brittle-ductile transition"],
        ),
        child(
            "Floating in balance",
            "Why high ground rides on deep foundations.",
            &[("isostasy", Some(2))],
            &["lithospheric root", "brittle-ductile transition"],
        ),
    ])]);

    let children = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    assert_eq!(children.len(), 3);
    assert_eq!(harness.provider.requests().len(), 1, "no repair request");
    assert_eq!(find(&harness.tree(), LITHOSPHERE).children.len(), 3);
}

#[tokio::test]
async fn the_request_carries_the_record_the_entry_the_concepts_the_conventions_the_titles_and_the_profile()
 {
    let harness = Harness::proposing(vec![answer(four_children())]);

    harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    let requests = harness.requests_for(Task::ProposeDeepDives);
    let request = &requests[0];
    assert!(request.schema.is_some(), "structured output");
    let input = &request.input;
    // The section's record.
    assert!(input.contains(LITHOSPHERE_SUMMARY), "{input}");
    assert!(
        input.contains("The rigid shell: crust plus uppermost mantle."),
        "{input}"
    );
    // Its map entry.
    assert!(input.contains("lithosphere, asthenosphere"), "{input}");
    assert!(input.contains("crust, mantle"), "{input}");
    // The concepts the course introduces before it, with their rungs.
    for concept in ["crust", "core", "asthenosphere"] {
        assert!(input.contains(concept), "{concept} in {input}");
    }
    // The conventions.
    assert!(input.contains("the Atlantic"), "{input}");
    assert!(
        input.contains("the weak, slowly flowing upper mantle beneath the lithosphere"),
        "{input}"
    );
    // The index titles.
    for title in ["Subduction zones", "Microplates", "Where plates meet"] {
        assert!(input.contains(title), "{title} in {input}");
    }
    // The profile.
    assert!(input.contains("working knowledge"), "{input}");
    assert!(
        input.contains("The mantle is a sea of liquid rock."),
        "{input}"
    );
}

// -- repairs ---------------------------------------------------------------------

/// Proposals the domain check refuses.
fn refused() -> Vec<(&'static str, String)> {
    let mut two = four_children();
    two.truncate(2);
    let mut seven = four_children();
    seven.extend([
        child(
            "Old ocean floor",
            "Why the oldest seafloor sits deepest.",
            &[("thermal subsidence", None)],
            &["lithosphere"],
        ),
        child(
            "Thin shells under oceans",
            "How the rigid shell thins where the sea floor is young.",
            &[("oceanic lithosphere", None)],
            &["lithosphere"],
        ),
        child(
            "Bending plates",
            "How the rigid shell flexes under a load.",
            &[("flexure", None)],
            &["lithosphere"],
        ),
    ]);
    let mut later = four_children();
    later[1] = child(
        "Roots under mountains",
        "How far down the rigid shell reaches beneath continents.",
        &[("lithospheric root", None)],
        &["tectonic plate"],
    );
    let mut nowhere = four_children();
    nowhere[2] = child(
        "Floating in balance",
        "Why high ground rides on deep foundations.",
        &[("isostasy", Some(2))],
        &["magma ocean"],
    );
    let mut later_sibling = four_children();
    later_sibling[0] = child(
        "Where rock stops breaking",
        "Why rock near the surface snaps and rock below flows.",
        &[("brittle-ductile transition", Some(2))],
        &["isostasy"],
    );
    let mut duplicate = four_children();
    duplicate[3]["title"] = json!(" Floating in balance ");
    let mut untitled = four_children();
    untitled[1]["title"] = json!("  ");
    vec![
        ("two proposals", answer(two)),
        ("seven proposals", answer(seven)),
        ("a concept introduced later in the course", answer(later)),
        ("a concept nothing introduces", answer(nowhere)),
        (
            "a concept a later sibling introduces",
            answer(later_sibling),
        ),
        ("a duplicate title", answer(duplicate)),
        ("an empty title", answer(untitled)),
    ]
}

#[tokio::test]
async fn refused_proposals_are_repaired_once_and_the_repair_is_stored() {
    for (case, refused) in refused() {
        let harness = Harness::proposing(vec![refused, answer(four_children())]);

        let children = harness.go_deeper(LITHOSPHERE).await;

        let children = children.unwrap_or_else(|error| panic!("{case}: {error:?}"));
        assert_eq!(titles(&children), four_titles(), "{case}");
        let tasks: Vec<Task> = harness
            .provider
            .requests()
            .iter()
            .map(|request| request.task)
            .collect();
        assert_eq!(
            tasks,
            [Task::ProposeDeepDives, Task::RepairStructured],
            "{case}"
        );
        assert_eq!(
            find(&harness.tree(), LITHOSPHERE).children.len(),
            4,
            "{case}"
        );
    }
}

#[tokio::test]
async fn a_second_refusal_is_reported_and_stores_no_children_and_no_map_entries() {
    for (case, refused) in refused() {
        let harness = Harness::proposing(vec![refused.clone(), refused]);
        let tree = harness.tree();
        let map = harness.map();

        let result = harness.go_deeper(LITHOSPHERE).await;

        assert!(
            matches!(result, Err(CoreError::Provider { .. })),
            "{case}: {result:?}"
        );
        assert_eq!(harness.provider.requests().len(), 2, "{case}");
        assert_eq!(harness.tree(), tree, "{case}");
        assert_eq!(harness.map(), map, "{case}");
        assert_eq!(harness.index_changes(), [], "{case}");
    }
}

// -- asking again ----------------------------------------------------------------

#[tokio::test]
async fn a_second_go_deeper_on_the_same_node_makes_no_request() {
    let harness = Harness::proposing(vec![answer(four_children())]);
    let first = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");
    harness.index_changes();

    let second = harness.go_deeper(LITHOSPHERE).await;

    assert_eq!(second, Ok(first));
    assert_eq!(harness.provider.requests().len(), 1);
    assert_eq!(harness.index_changes(), []);
    assert_eq!(find(&harness.tree(), LITHOSPHERE).children.len(), 4);
}

#[tokio::test]
async fn a_node_with_children_already_answers_them_with_no_request() {
    let harness = Harness::new(|provider| provider);

    let children = harness
        .go_deeper(WHAT_A_PLATE_IS)
        .await
        .expect("deep dives");

    assert_eq!(
        ids(&children),
        [HOW_PLATES_ARE_MAPPED, PLATE_THICKNESS, MICROPLATES]
    );
    assert_eq!(
        titles(&children),
        ["How plates are mapped", "Plate thickness", "Microplates"]
    );
    assert!(harness.provider.requests().is_empty());
    assert_eq!(harness.index_changes(), []);
}

// -- opening the children --------------------------------------------------------

#[tokio::test]
async fn a_child_is_written_from_its_entry_and_its_parent_record_and_a_grandchild_from_its_own_parent()
 {
    let harness = Harness::new(|provider| {
        provider
            .answer(
                Task::ProposeDeepDives,
                [
                    Ok(answer(four_children())),
                    Ok(answer(three_grandchildren())),
                ],
            )
            .answer(
                Task::WriteSection,
                [Ok(fixture("complete.md")), Ok(fixture("complete.md"))],
            )
    });
    let children = harness.go_deeper(LITHOSPHERE).await.expect("deep dives");

    harness.open(children[0].id).await.expect("the child opens");

    let requests = harness.requests_for(Task::WriteSection);
    assert_eq!(requests.len(), 1);
    let input = &requests[0].input;
    let entry = part(input, "# This section: ");
    assert!(entry.contains("1.2.1 Where rock stops breaking"), "{entry}");
    assert!(
        entry.contains("Introduces: brittle-ductile transition"),
        "{entry}"
    );
    assert!(
        entry.contains("Relies on: lithosphere, asthenosphere"),
        "{entry}"
    );
    let parent = part(input, "# Parent section: ");
    assert!(
        parent.contains("1.2 Lithosphere and asthenosphere"),
        "{parent}"
    );
    assert!(parent.contains(LITHOSPHERE_SUMMARY), "{parent}");

    // The child, now written, is deepened the same way.
    let grandchildren = harness.go_deeper(children[0].id).await.expect("deeper");
    assert_eq!(
        titles(&grandchildren),
        [
            "Earthquakes stop at depth",
            "Hot rock, weak rock",
            "Water and weakness",
        ]
    );
    assert_eq!(
        find(&harness.tree(), children[0].id).children.len(),
        3,
        "stored under the child"
    );

    harness
        .open(grandchildren[0].id)
        .await
        .expect("the grandchild opens");

    let requests = harness.requests_for(Task::WriteSection);
    assert_eq!(requests.len(), 2);
    let input = &requests[1].input;
    let entry = part(input, "# This section: ");
    assert!(
        entry.contains("1.2.1.1 Earthquakes stop at depth"),
        "{entry}"
    );
    assert!(entry.contains("Introduces: seismogenic depth"), "{entry}");
    let parent = part(input, "# Parent section: ");
    assert!(
        parent.contains("1.2.1 Where rock stops breaking"),
        "{parent}"
    );
    assert!(parent.contains(WRITTEN_SUMMARY), "{parent}");
    assert!(!parent.contains(LITHOSPHERE_SUMMARY), "{parent}");
}

// -- refusals --------------------------------------------------------------------

#[tokio::test]
async fn go_deeper_on_a_section_never_generated_is_refused() {
    let harness = Harness::proposing(vec![answer(four_children())]);
    let tree = harness.tree();
    let map = harness.map();

    let result = harness.go_deeper(WHAT_DRIVES_PLATES).await;

    assert!(
        matches!(result, Err(CoreError::InvalidRequest { .. })),
        "{result:?}"
    );
    assert!(harness.provider.requests().is_empty());
    assert_eq!(harness.tree(), tree);
    assert_eq!(harness.map(), map);
}

#[tokio::test]
async fn go_deeper_on_a_node_not_in_the_course_is_not_found() {
    let harness = Harness::proposing(vec![answer(four_children())]);

    let result = harness.go_deeper(Uuid::from_u128(0xdead)).await;

    assert!(
        matches!(result, Err(CoreError::NotFound { .. })),
        "{result:?}"
    );
    assert!(harness.provider.requests().is_empty());
}
