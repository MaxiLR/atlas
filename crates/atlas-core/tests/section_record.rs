//! The section record at the boundary between the model's output and what
//! later prompts carry: `validate` on records written as JSON, `resolve` with
//! a scripted fake provider (one repair request, then the predicted record),
//! and `apply_conventions` keeping each convention's first fixing.
//!
//! Expected records are written out by hand, never derived with the code under
//! test.

mod support;

use atlas_core::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use atlas_core::domain::course_map::MapEntry;
use atlas_core::domain::record::{
    FixedEntry, Promise, RecordStatus, SectionRecord, StoredRecord, TaughtConcept,
};
use atlas_core::harness::record::{
    MAX_ENTRIES, MAX_LINE_CHARS, MAX_RECORD_CHARS, MAX_SUMMARY_CHARS, RecordFault,
    apply_conventions, resolve, validate,
};
use atlas_core::provider::{FakeProvider, FakeScript, ProviderError, ProviderRequest, Task};
use serde_json::{Value, json};

use support::sample_course::{LITHOSPHERE, SampleCourse, WHAT_A_PLATE_IS_V1, sample_course};

const MODEL: &str = "gpt-test";

/// The sample course's sections in index order, with their index keys.
const KEYS: [&str; 9] = [
    "1.1", "1.2", "2.1", "2.1.1", "2.1.2", "2.1.3", "2.2", "3.1", "3.2",
];

/// The prose of "Lithosphere and asthenosphere", the section resolved below.
const SECTION_TEXT: &str = "Sort the outer Earth by strength instead of by \
    composition and a new picture appears: the crust and the top of the mantle \
    act as one rigid shell, the lithosphere, floating on a softer layer that \
    creeps like warm wax.";

/// The prose of other sections, never sent with this one's repair.
const OTHER_SECTION_TEXTS: [&str; 2] = [
    "Cut the Earth open like a peach and three layers show: a thin skin of \
     crust, a thick flesh of mantle and a metal stone at the centre.",
    "Plates are pieces of that rigid shell, and each moves as one, a few \
     centimetres a year.",
];

/// The keys of the sections after `key` in index order.
fn later_than(key: &str) -> Vec<String> {
    let at = KEYS
        .iter()
        .position(|&k| k == key)
        .unwrap_or_else(|| panic!("no section {key}"));
    KEYS[at + 1..].iter().map(|&k| k.to_owned()).collect()
}

fn course() -> SampleCourse {
    sample_course()
}

fn lithosphere_entry() -> MapEntry {
    course()
        .map
        .entries
        .into_iter()
        .find(|entry| entry.section == LITHOSPHERE)
        .expect("the sample map has an entry for the lithosphere section")
}

/// The record "Lithosphere and asthenosphere" writes, as JSON.
fn lithosphere_json() -> Value {
    json!({
        "summary": "By strength, the crust and the top of the mantle form one rigid \
            shell, the lithosphere. It rides on the asthenosphere, which flows slowly.",
        "concepts": [
            {"name": "lithosphere", "line": "The rigid shell: crust plus uppermost mantle."},
            {"name": "asthenosphere", "line": "The soft layer below, solid but flowing."}
        ],
        "notation": [],
        "terms": [
            {
                "key": "asthenosphere",
                "meaning": "the weak, slowly flowing upper mantle beneath the lithosphere"
            }
        ],
        "examples": ["ice floes on a pond"],
        "promises": [{"section": "2.1", "what": "what a single plate is"}]
    })
}

/// [`lithosphere_json`] as typed values, written out by hand.
fn lithosphere_record() -> SectionRecord {
    SectionRecord {
        summary: Some(
            "By strength, the crust and the top of the mantle form one rigid shell, the \
             lithosphere. It rides on the asthenosphere, which flows slowly."
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
        examples: vec!["ice floes on a pond".to_owned()],
        promises: vec![Promise {
            section: "2.1".to_owned(),
            what: "what a single plate is".to_owned(),
        }],
    }
}

/// The record the lithosphere section's map entry predicts, written out.
fn lithosphere_predicted() -> StoredRecord {
    let planned = |name: &str| TaughtConcept {
        name: name.to_owned(),
        line: None,
    };
    StoredRecord {
        record: SectionRecord {
            summary: None,
            concepts: vec![planned("lithosphere"), planned("asthenosphere")],
            notation: Vec::new(),
            terms: Vec::new(),
            examples: Vec::new(),
            promises: Vec::new(),
        },
        status: RecordStatus::Predicted,
    }
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

fn with(mut value: Value, path: &str, replacement: Value) -> Value {
    *value
        .pointer_mut(path)
        .unwrap_or_else(|| panic!("no {path} in the record")) = replacement;
    value
}

fn without(mut value: Value, field: &str) -> Value {
    value
        .as_object_mut()
        .expect("a record is an object")
        .remove(field);
    value
}

fn refused(payload: &str) -> RecordFault {
    match validate(payload, &later_than("1.2")) {
        Ok(record) => panic!("{payload} was accepted as {record:?}"),
        Err(fault) => fault,
    }
}

fn assert_refused_naming(payload: &Value, word: &str) {
    let fault = refused(&payload.to_string());
    assert!(
        fault.reason.to_lowercase().contains(word),
        "the reason for {payload} does not name {word:?}: {}",
        fault.reason
    );
}

fn provider() -> FakeProvider {
    FakeProvider::new(FakeScript::sample())
}

fn ok(text: &str) -> Result<String, ProviderError> {
    Ok(text.to_owned())
}

fn sent_text(request: &ProviderRequest) -> String {
    format!("{}\n{}", request.instructions, request.input)
}

async fn resolved(raw: Option<&str>, fake: &FakeProvider) -> StoredRecord {
    resolve(
        raw,
        SECTION_TEXT,
        &lithosphere_entry(),
        &later_than("1.2"),
        fake,
        Some(MODEL),
    )
    .await
}

/// The one repair request `fake` received, after checking it was the only
/// request and a record repair.
fn only_repair(fake: &FakeProvider) -> ProviderRequest {
    let requests = fake.requests();
    assert_eq!(requests.len(), 1, "exactly one repair request");
    let request = requests.into_iter().next().unwrap();
    assert_eq!(request.task, Task::RepairRecord);
    assert_eq!(request.model.as_deref(), Some(MODEL));
    request
}

// --- validate ---------------------------------------------------------------

#[test]
fn the_sample_course_records_are_valid_and_come_back_equal() {
    let course = course();
    let keys: Vec<(uuid::Uuid, &str)> = vec![
        (support::sample_course::LAYERS, "1.1"),
        (support::sample_course::LITHOSPHERE, "1.2"),
        (support::sample_course::WHAT_A_PLATE_IS, "2.1"),
        (support::sample_course::HOW_PLATES_ARE_MAPPED, "2.1.1"),
    ];
    let mut checked = 0;
    for sample in &course.records {
        if sample.record.status != RecordStatus::Confirmed {
            continue;
        }
        let key = keys
            .iter()
            .find(|(section, _)| *section == sample.section)
            .map(|(_, key)| *key)
            .expect("every sample section read has a key");
        let payload = serde_json::to_string_pretty(&sample.record.record).unwrap();
        let record = validate(&payload, &later_than(key))
            .unwrap_or_else(|fault| panic!("section {key} was refused: {fault}"));
        assert_eq!(record, sample.record.record, "section {key}");
        checked += 1;
    }
    assert_eq!(checked, 3, "the sample course has three confirmed records");
}

#[test]
fn a_record_written_by_hand_is_read_as_written() {
    let record = validate(&lithosphere_json().to_string(), &later_than("1.2")).unwrap();
    assert_eq!(record, lithosphere_record());
}

#[test]
fn a_payload_that_is_not_json_is_invalid_with_a_reason() {
    let fault = refused("The section taught the lithosphere.");
    assert!(
        fault.reason.contains("JSON"),
        "the reason does not say the payload is not JSON: {}",
        fault.reason
    );
}

#[test]
fn json_of_another_shape_is_invalid_with_a_reason() {
    for payload in [json!([1, 2, 3]), json!("a summary"), json!({"title": "x"})] {
        let fault = refused(&payload.to_string());
        assert!(!fault.reason.trim().is_empty(), "{payload}");
    }
}

#[test]
fn a_missing_or_empty_summary_is_invalid_with_a_reason() {
    assert_refused_naming(&without(lithosphere_json(), "summary"), "summary");
    assert_refused_naming(
        &with(lithosphere_json(), "/summary", Value::Null),
        "summary",
    );
    assert_refused_naming(
        &with(lithosphere_json(), "/summary", json!("  ")),
        "summary",
    );
}

#[test]
fn an_oversized_summary_is_invalid_with_a_reason() {
    let at_limit = "a".repeat(MAX_SUMMARY_CHARS);
    let payload = with(lithosphere_json(), "/summary", json!(at_limit));
    let record = validate(&payload.to_string(), &later_than("1.2")).unwrap();
    assert_eq!(record.summary.as_deref(), Some(at_limit.as_str()));

    let oversized = "é".repeat(MAX_SUMMARY_CHARS + 1);
    assert_refused_naming(
        &with(lithosphere_json(), "/summary", json!(oversized)),
        "summary",
    );
}

#[test]
fn an_empty_or_missing_concept_list_is_invalid_with_a_reason() {
    assert_refused_naming(&with(lithosphere_json(), "/concepts", json!([])), "concept");
    assert_refused_naming(&without(lithosphere_json(), "concepts"), "concept");
}

#[test]
fn a_concept_without_a_name_or_a_line_is_invalid_with_a_reason() {
    assert_refused_naming(
        &with(lithosphere_json(), "/concepts/0/name", json!("")),
        "name",
    );
    assert_refused_naming(
        &with(lithosphere_json(), "/concepts/0/line", Value::Null),
        "line",
    );
    assert_refused_naming(
        &with(
            lithosphere_json(),
            "/concepts/1/line",
            json!("x".repeat(MAX_LINE_CHARS + 1)),
        ),
        "line",
    );
}

#[test]
fn a_term_or_symbol_without_a_meaning_is_invalid_with_a_reason() {
    assert_refused_naming(
        &with(
            lithosphere_json(),
            "/terms/0",
            json!({"key": "asthenosphere"}),
        ),
        "meaning",
    );
    assert_refused_naming(
        &with(lithosphere_json(), "/terms/0/meaning", json!(" ")),
        "meaning",
    );
    assert_refused_naming(
        &with(
            lithosphere_json(),
            "/notation",
            json!([{"key": "h", "meaning": ""}]),
        ),
        "meaning",
    );
    assert_refused_naming(
        &with(
            lithosphere_json(),
            "/notation",
            json!([{"key": "", "meaning": "height"}]),
        ),
        "key",
    );
}

#[test]
fn an_example_must_be_a_short_text() {
    assert_refused_naming(
        &with(lithosphere_json(), "/examples", json!([""])),
        "example",
    );
    assert_refused_naming(
        &with(
            lithosphere_json(),
            "/examples",
            json!(["x".repeat(MAX_LINE_CHARS + 1)]),
        ),
        "example",
    );
}

#[test]
fn a_promise_must_say_what_was_promised() {
    assert_refused_naming(
        &with(
            lithosphere_json(),
            "/promises",
            json!([{"section": "2.1", "what": ""}]),
        ),
        "promise",
    );
}

#[test]
fn too_many_entries_or_too_large_a_record_is_invalid() {
    let concepts: Vec<Value> = (0..=MAX_ENTRIES)
        .map(|at| json!({"name": format!("concept {at}"), "line": "A line."}))
        .collect();
    assert_refused_naming(
        &with(lithosphere_json(), "/concepts", json!(concepts)),
        "concept",
    );

    // Every entry within its own limits, the whole record over its limit.
    let filler = "x".repeat(MAX_LINE_CHARS - 20);
    let concepts: Vec<Value> = (0..MAX_ENTRIES)
        .map(|at| json!({"name": format!("concept {at}"), "line": filler}))
        .collect();
    let examples: Vec<Value> = (0..MAX_ENTRIES)
        .map(|at| json!(format!("example {at} {filler}")))
        .collect();
    let payload = with(
        with(lithosphere_json(), "/examples", json!(examples)),
        "/concepts",
        json!(concepts),
    )
    .to_string();
    assert!(payload.chars().count() > MAX_RECORD_CHARS);
    let fault = refused(&payload);
    assert!(fault.reason.contains("record"), "{}", fault.reason);
}

#[test]
fn a_promise_to_an_unknown_or_earlier_section_is_dropped_and_the_rest_kept() {
    let payload = with(
        lithosphere_json(),
        "/promises",
        json!([
            {"section": "1.1", "what": "the layers again"},
            {"section": "2.1", "what": "what a single plate is"},
            {"section": "9.9", "what": "a chapter that does not exist"},
            {"section": "1.2", "what": "this very section"}
        ]),
    );
    let record = validate(&payload.to_string(), &later_than("1.2")).unwrap();
    assert_eq!(record, lithosphere_record());
}

#[test]
fn the_record_is_not_compared_with_the_map_entry() {
    let payload = with(
        lithosphere_json(),
        "/concepts",
        json!([{"name": "isostasy", "line": "Light crust floats higher."}]),
    );
    let record = validate(&payload.to_string(), &later_than("1.2")).unwrap();
    assert_eq!(
        record.concepts,
        vec![taught("isostasy", "Light crust floats higher.")]
    );
}

// --- resolve ----------------------------------------------------------------

#[tokio::test]
async fn a_valid_record_is_confirmed_without_a_request() {
    let fake = provider();
    let stored = resolved(Some(&lithosphere_json().to_string()), &fake).await;
    assert_eq!(
        stored,
        StoredRecord {
            record: lithosphere_record(),
            status: RecordStatus::Confirmed,
        }
    );
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn an_invalid_record_is_repaired_once_and_confirmed() {
    let invalid = without(lithosphere_json(), "summary").to_string();
    let reason = refused(&invalid).reason;
    let fake = provider().answer(
        Task::RepairRecord,
        [ok(&lithosphere_json().to_string()), ok("never asked for")],
    );

    let stored = resolved(Some(&invalid), &fake).await;

    assert_eq!(
        stored,
        StoredRecord {
            record: lithosphere_record(),
            status: RecordStatus::Confirmed,
        }
    );
    let sent = sent_text(&only_repair(&fake));
    assert!(sent.contains(SECTION_TEXT), "the section's text: {sent}");
    assert!(sent.contains(&invalid), "the invalid payload: {sent}");
    assert!(sent.contains(&reason), "the reason {reason:?}: {sent}");
    for concept in ["lithosphere", "asthenosphere", "crust", "mantle"] {
        assert!(sent.contains(concept), "the map entry's {concept}: {sent}");
    }
}

#[tokio::test]
async fn a_repair_answered_in_a_code_fence_is_read() {
    let fake = provider().answer(
        Task::RepairRecord,
        [ok(&format!("```json\n{}\n```", lithosphere_json()))],
    );
    let stored = resolved(Some("not a record"), &fake).await;
    assert_eq!(stored.status, RecordStatus::Confirmed);
    assert_eq!(stored.record, lithosphere_record());
}

#[tokio::test]
async fn invalid_twice_gives_the_predicted_record_after_one_repair() {
    let invalid = with(lithosphere_json(), "/concepts", json!([])).to_string();
    let fake = provider().answer(
        Task::RepairRecord,
        [ok(&invalid), ok(&lithosphere_json().to_string())],
    );

    let stored = resolved(Some(&invalid), &fake).await;

    assert_eq!(stored, lithosphere_predicted());
    only_repair(&fake);
}

#[tokio::test]
async fn a_missing_record_whose_repair_is_invalid_gives_the_predicted_record() {
    let fake = provider().answer(
        Task::RepairRecord,
        [ok("{\"summary\": \"half a record\""), ok("{}")],
    );

    let stored = resolved(None, &fake).await;

    assert_eq!(stored, lithosphere_predicted());
    let sent = sent_text(&only_repair(&fake));
    assert!(sent.contains(SECTION_TEXT), "the section's text: {sent}");
}

#[tokio::test]
async fn a_failed_repair_request_gives_the_predicted_record() {
    let fake = provider().answer(
        Task::RepairRecord,
        [
            Err(ProviderError::PlanLimit {
                resets_at: None,
                request_id: Some("req_1".to_owned()),
            }),
            ok(&lithosphere_json().to_string()),
        ],
    );

    let stored = resolved(Some("{\"summary\": 3}"), &fake).await;

    assert_eq!(stored, lithosphere_predicted());
    only_repair(&fake);
}

#[tokio::test]
async fn the_repair_request_contains_no_other_section_text() {
    let course = course();
    let fake = provider().answer(Task::RepairRecord, [ok("{}")]);

    resolved(Some("not a record"), &fake).await;

    let sent = sent_text(&only_repair(&fake));
    let mut others: Vec<String> = OTHER_SECTION_TEXTS.map(str::to_owned).to_vec();
    for sample in course.records.iter().filter(|s| s.section != LITHOSPHERE) {
        let record = &sample.record.record;
        others.extend(record.summary.clone());
        others.extend(record.concepts.iter().filter_map(|c| c.line.clone()));
        others.extend(record.examples.iter().cloned());
        others.extend(record.promises.iter().map(|p| p.what.clone()));
    }
    for chapter in &course.index.chapters {
        for section in &chapter.sections {
            if section.id != LITHOSPHERE {
                others.push(section.line.clone());
                others.extend(section.children.iter().map(|child| child.line.clone()));
            }
        }
    }
    for other in &others {
        assert!(
            !sent.contains(other.as_str()),
            "the repair request carries {other:?}"
        );
    }
}

// --- apply_conventions ------------------------------------------------------

fn origin() -> ConventionOrigin {
    ConventionOrigin::SectionVersion {
        version: WHAT_A_PLATE_IS_V1,
    }
}

#[test]
fn a_redefined_symbol_is_removed_and_a_new_term_is_reported() {
    let existing = course().conventions;
    let before = existing.clone();
    let record = SectionRecord {
        notation: vec![
            fixed("V", "the volume of a plate, in cubic kilometres"),
            fixed("h", "a plate's thickness, in kilometres"),
        ],
        terms: vec![
            fixed("slab", "the sinking part of a plate"),
            fixed(
                "asthenosphere",
                "the weak, slowly flowing upper mantle beneath the lithosphere",
            ),
        ],
        ..lithosphere_record()
    };

    let applied = apply_conventions(record.clone(), &existing, origin());

    assert_eq!(existing, before, "the first fixing is untouched");
    assert_eq!(
        applied.record,
        SectionRecord {
            notation: vec![fixed("h", "a plate's thickness, in kilometres")],
            ..record
        },
        "only the conflicting symbol is removed"
    );
    let new_keys: Vec<(ConventionKind, &str)> = applied
        .fixed
        .iter()
        .filter(|c| c.kind != ConventionKind::RunningExample)
        .map(|c| (c.kind, c.key.as_str()))
        .collect();
    assert_eq!(
        new_keys,
        vec![
            (ConventionKind::Notation, "h"),
            (ConventionKind::Terminology, "slab"),
        ]
    );
    assert!(
        applied.fixed.contains(&Convention {
            kind: ConventionKind::Terminology,
            key: "slab".to_owned(),
            meaning: "the sinking part of a plate".to_owned(),
            origin: origin(),
        }),
        "{:?}",
        applied.fixed
    );
}

#[test]
fn a_record_conflicting_with_itself_keeps_its_first_fixing() {
    let record = SectionRecord {
        terms: vec![
            fixed("ridge", "where plates pull apart"),
            fixed("Ridge", "a mountain chain on land"),
        ],
        ..lithosphere_record()
    };

    let applied = apply_conventions(record, &[], origin());

    assert_eq!(
        applied.record.terms,
        vec![fixed("ridge", "where plates pull apart")]
    );
    let terms: Vec<&Convention> = applied
        .fixed
        .iter()
        .filter(|c| c.kind == ConventionKind::Terminology)
        .collect();
    assert_eq!(terms.len(), 1);
    assert_eq!(terms[0].meaning, "where plates pull apart");
}

#[test]
fn running_examples_are_offered_as_conventions() {
    let existing = course().conventions;
    let record = SectionRecord {
        examples: vec!["the Atlantic".to_owned(), "a conveyor belt".to_owned()],
        ..lithosphere_record()
    };

    let applied = apply_conventions(record.clone(), &existing, origin());

    assert_eq!(applied.record.examples, record.examples);
    let examples: Vec<&str> = applied
        .fixed
        .iter()
        .filter(|c| c.kind == ConventionKind::RunningExample)
        .map(|c| c.key.as_str())
        .collect();
    assert_eq!(examples, vec!["a conveyor belt"]);
    assert!(applied.fixed.iter().all(|c| c.origin == origin()));
}
