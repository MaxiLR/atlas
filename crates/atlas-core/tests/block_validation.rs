//! Block validation at the boundary between the model's output and the
//! blocks a candidate sees: `validate` on fixtures, and `resolve` with a
//! scripted fake provider (one repair request, then the text fallback).
//!
//! The inertness check on figures is written independently of the sanitiser:
//! it re-parses the SVG and looks for what the spec forbids.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use atlas_core::domain::ladder::{Ladder, RungId};
use atlas_core::domain::questions::{KeyedCheckQuestion, KeyedOption};
use atlas_core::harness::splitter::ReservedKind;
use atlas_core::harness::validate::{
    NEUTRAL_DESCRIPTION, RawBlock, RecordBlock, Resolved, ValidBlock, resolve, validate,
};
use atlas_core::protocol::{BlockContent, CheckQuestion};
use atlas_core::provider::{FakeProvider, FakeScript, ProviderError, ProviderRequest, Task};
use atlas_core::sanitise::sanitise;
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use serde_json::{Value, json};

use support::sample_course::sample_course;

const MODEL: &str = "gpt-test";
const DESCRIPTION: &str = "Where the sinking plate goes";

const MERMAID: &str = "flowchart LR\n  A[Oceanic plate] --> B[Trench]\n  B --> C[Mantle]";
const SIMPLE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 40\">\
    <path d=\"M0 20H100\" stroke=\"currentColor\"/></svg>";

fn ladder() -> Ladder {
    sample_course().ladder
}

fn manifest(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn files(folder: &str, extension: &str) -> Vec<PathBuf> {
    let dir = manifest(folder);
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {}", dir.display());
    paths
}

fn closed(kind: ReservedKind, payload: &str) -> RawBlock {
    RawBlock::closed(kind, payload, Some(DESCRIPTION.to_owned())).expect("a shown block")
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

async fn resolved(raw: &RawBlock, fake: &FakeProvider) -> Resolved {
    resolve(raw, &ladder(), fake, Some(MODEL), &AtomicBool::new(false)).await
}

// --- Check questions --------------------------------------------------------

fn question_json(rung: u32) -> Value {
    json!({
        "rung": rung,
        "stem": "A young, warm oceanic plate meets an old, cold one. Which sinks?",
        "options": [
            { "text": "The old, cold plate", "misconception": null },
            { "text": "The young, warm plate", "misconception": "Thinks warmth makes a plate sink" },
            { "text": "Neither: they build mountains", "misconception": "Confuses the boundary types" },
            { "text": "They slide past each other", "misconception": "Confuses convergent with transform" }
        ],
        "key": 0,
        "explanation": "Oceanic lithosphere grows denser as it ages and cools."
    })
}

fn second_question_json() -> Value {
    json!({
        "rung": 3,
        "stem": "What is a tectonic plate made of?",
        "options": [
            { "text": "The crust alone", "misconception": "Leaves out the upper mantle" },
            { "text": "Crust and the rigid upper mantle", "misconception": null },
            { "text": "The asthenosphere", "misconception": "Confuses the rigid and the soft layers" },
            { "text": "The outer core", "misconception": "Places plates far too deep" }
        ],
        "key": 1,
        "explanation": "A plate is a piece of the lithosphere: crust plus rigid upper mantle."
    })
}

fn check_payload(questions: Vec<Value>) -> String {
    json!({ "questions": questions }).to_string()
}

fn keyed(
    rung: u32,
    stem: &str,
    options: [(&str, Option<&str>); 4],
    key: u32,
    explanation: &str,
) -> KeyedCheckQuestion {
    KeyedCheckQuestion {
        rung: RungId(rung),
        stem: stem.to_owned(),
        options: options
            .into_iter()
            .map(|(text, misconception)| KeyedOption {
                text: text.to_owned(),
                misconception: misconception.map(str::to_owned),
            })
            .collect(),
        key,
        explanation: explanation.to_owned(),
    }
}

fn expected_first(rung: u32) -> KeyedCheckQuestion {
    keyed(
        rung,
        "A young, warm oceanic plate meets an old, cold one. Which sinks?",
        [
            ("The old, cold plate", None),
            (
                "The young, warm plate",
                Some("Thinks warmth makes a plate sink"),
            ),
            (
                "Neither: they build mountains",
                Some("Confuses the boundary types"),
            ),
            (
                "They slide past each other",
                Some("Confuses convergent with transform"),
            ),
        ],
        0,
        "Oceanic lithosphere grows denser as it ages and cools.",
    )
}

fn expected_second() -> KeyedCheckQuestion {
    keyed(
        3,
        "What is a tectonic plate made of?",
        [
            ("The crust alone", Some("Leaves out the upper mantle")),
            ("Crust and the rigid upper mantle", None),
            (
                "The asthenosphere",
                Some("Confuses the rigid and the soft layers"),
            ),
            ("The outer core", Some("Places plates far too deep")),
        ],
        1,
        "A plate is a piece of the lithosphere: crust plus rigid upper mantle.",
    )
}

// --- An independent inertness check for SVG ---------------------------------

/// What the spec forbids in a figure, found by re-parsing it: `script`,
/// `foreignObject`, event attributes, external references and `url()` that
/// names anything but a local definition.
fn forbidden_in(svg: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut reader = Reader::from_str(svg);
    loop {
        match reader.read_event() {
            Err(error) => {
                found.push(format!("not well-formed: {error}"));
                break;
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(element) | Event::Empty(element)) => {
                let name = (element.local_name().as_ref() as &str).to_lowercase();
                if [
                    "script",
                    "foreignobject",
                    "iframe",
                    "image",
                    "use",
                    "a",
                    "style",
                ]
                .contains(&name.as_str())
                {
                    found.push(format!("element {name}"));
                }
                for attribute in element.attributes() {
                    let attribute = attribute.expect("attributes parse");
                    let key = (attribute.key.as_ref() as &str).to_lowercase();
                    let value = attribute
                        .normalized_value(XmlVersion::Implicit1_0)
                        .expect("attribute values decode")
                        .to_lowercase();
                    if key.starts_with("on") {
                        found.push(format!("event attribute {key}"));
                    }
                    if key.ends_with("href") || key == "src" {
                        found.push(format!("reference {key}"));
                    }
                    for bad in ["javascript:", "data:", "@import", "expression("] {
                        if value.contains(bad) {
                            found.push(format!("{bad} in {key}"));
                        }
                    }
                    let mut rest = value.as_str();
                    while let Some(at) = rest.find("url(") {
                        rest = &rest[at + 4..];
                        if !rest.trim_start_matches(['"', '\'', ' ']).starts_with('#') {
                            found.push(format!("external url() in {key}"));
                        }
                    }
                }
            }
            Ok(Event::PI(_)) => found.push("processing instruction".to_owned()),
            Ok(Event::DocType(_)) => found.push("doctype".to_owned()),
            Ok(Event::CData(_)) => found.push("a CDATA section".to_owned()),
            Ok(_) => {}
        }
    }
    found
}

// --- validate: what passes ---------------------------------------------------

/// Every fixture comes out as it went in, but for a figure entry's SVG, which
/// comes out as the sanitiser writes it.
#[test]
fn each_valid_catalog_fixture_passes_unchanged_with_its_figure_sanitised() {
    for path in files("tests/fixtures/catalog/valid", "json") {
        let text = read(&path);
        let mut spec: Value = serde_json::from_str(&text).unwrap();
        let root = spec["root"].as_str().unwrap().to_owned();
        let props = &mut spec["elements"][&root]["props"];
        if let Some(svg) = props["svg"].as_str() {
            let sanitised = sanitise(svg).expect("the fixture's figure is allowed");
            props["svg"] = Value::String(sanitised.as_str().to_owned());
        }

        let block = validate(&closed(ReservedKind::Visual, &text), &ladder());

        assert_eq!(block, Ok(ValidBlock::Visual { spec }), "{}", path.display());
    }
}

#[test]
fn an_allowed_figure_passes_and_becomes_the_sanitised_svg() {
    for path in files("tests/fixtures/svg/allowed", "svg") {
        let svg = read(&path);
        let expected = sanitise(&svg).expect("an allowed figure");

        let block = validate(&closed(ReservedKind::Figure, &svg), &ladder());

        assert_eq!(
            block,
            Ok(ValidBlock::Figure { svg: expected }),
            "{}",
            path.display()
        );
    }
}

#[test]
fn a_mermaid_diagram_passes_unchanged() {
    let block = validate(&closed(ReservedKind::Mermaid, MERMAID), &ladder());

    assert_eq!(
        block,
        Ok(ValidBlock::Mermaid {
            source: MERMAID.to_owned()
        })
    );
}

#[test]
fn one_or_two_check_questions_pass_with_their_keys() {
    let one = check_payload(vec![question_json(7)]);
    let two = check_payload(vec![question_json(7), second_question_json()]);

    assert_eq!(
        validate(&closed(ReservedKind::Check, &one), &ladder()),
        Ok(ValidBlock::Check {
            questions: vec![expected_first(7)]
        })
    );
    assert_eq!(
        validate(&closed(ReservedKind::Check, &two), &ladder()),
        Ok(ValidBlock::Check {
            questions: vec![expected_first(7), expected_second()]
        })
    );
}

// --- validate: what is refused ----------------------------------------------

fn assert_invalid(kind: ReservedKind, payload: &str, case: &str) {
    let result = validate(&closed(kind, payload), &ladder());
    let fault = result.expect_err(case);
    assert!(!fault.reason.trim().is_empty(), "{case}: an empty reason");
}

#[test]
fn check_questions_naming_an_unknown_rung_are_invalid() {
    assert_invalid(
        ReservedKind::Check,
        &check_payload(vec![question_json(42)]),
        "rung 42 is not on the ladder",
    );
    assert_invalid(
        ReservedKind::Check,
        &check_payload(vec![second_question_json(), question_json(0)]),
        "rung 0 is not on the ladder",
    );
}

#[test]
fn check_questions_carrying_two_keys_are_invalid() {
    let mut two_null = question_json(7);
    two_null["options"][1]["misconception"] = Value::Null;
    assert_invalid(
        ReservedKind::Check,
        &check_payload(vec![two_null]),
        "two options marked right",
    );

    let mut two_keys = question_json(7);
    two_keys["key"] = json!([0, 1]);
    assert_invalid(
        ReservedKind::Check,
        &check_payload(vec![two_keys]),
        "a key naming two options",
    );
}

#[test]
fn check_questions_need_one_or_two_questions_of_four_options() {
    assert_invalid(ReservedKind::Check, &check_payload(vec![]), "no question");
    assert_invalid(
        ReservedKind::Check,
        &check_payload(vec![
            question_json(7),
            second_question_json(),
            question_json(5),
        ]),
        "three questions",
    );
    let mut three_options = question_json(7);
    three_options["options"].as_array_mut().unwrap().pop();
    assert_invalid(
        ReservedKind::Check,
        &check_payload(vec![three_options]),
        "three options",
    );
    assert_invalid(ReservedKind::Check, "Which plate sinks?", "not JSON");
}

#[test]
fn a_mermaid_payload_with_an_init_directive_or_a_click_line_is_invalid() {
    let cases = [
        (
            "%%{init: {\"securityLevel\": \"loose\"}}%%\nflowchart LR\n  A --> B",
            "an init directive",
        ),
        (
            "flowchart LR\n  A --> B\n  %%{initialize: {\"theme\": \"dark\"}}%%",
            "an initialize directive further down",
        ),
        (
            "flowchart LR\n  A --> B\n  click A href \"https://evil.example\"",
            "a click line with a link",
        ),
        (
            "flowchart LR\n  A --> B\nclick B callback",
            "a click line with a callback",
        ),
        (
            "sequenceDiagram\n  Alice->>Bob: Hi\n  link Alice: Home @ https://evil.example",
            "a sequence-diagram link",
        ),
        ("", "an empty diagram"),
        ("  \n\t\n", "a blank diagram"),
    ];
    for (payload, case) in cases {
        assert_invalid(ReservedKind::Mermaid, payload, case);
    }
    let oversized = format!("flowchart LR\n{}", "  A --> B\n".repeat(10_000));
    assert_invalid(ReservedKind::Mermaid, &oversized, "an oversized diagram");
}

#[test]
fn an_invalid_catalog_visual_is_invalid() {
    for path in files("tests/fixtures/catalog/invalid", "json") {
        assert_invalid(
            ReservedKind::Visual,
            &read(&path),
            &path.display().to_string(),
        );
    }
    assert_invalid(ReservedKind::Visual, "a concept map of plates", "not JSON");
}

#[test]
fn a_figure_that_the_sanitiser_refuses_is_invalid() {
    for name in ["not-svg.svg", "nothing-drawable.svg", "oversized.svg"] {
        let svg = read(&manifest("tests/fixtures/svg/hostile").join(name));
        assert_invalid(ReservedKind::Figure, &svg, name);
    }
}

#[test]
fn a_hostile_figure_never_yields_a_figure_with_anything_forbidden() {
    for path in files("tests/fixtures/svg/hostile", "svg") {
        let svg = read(&path);
        if let Ok(ValidBlock::Figure { svg }) =
            validate(&closed(ReservedKind::Figure, &svg), &ladder())
        {
            let found = forbidden_in(svg.as_str());
            assert!(
                found.is_empty(),
                "{}: {found:?}\n{}",
                path.display(),
                svg.as_str()
            );
        }
    }
}

#[tokio::test]
async fn a_hostile_repair_of_a_figure_never_yields_a_figure_with_anything_forbidden() {
    let hostile = read(&manifest("tests/fixtures/svg/hostile/script-element.svg"));
    let fake = provider().answer(Task::RepairBlock, [ok(&hostile)]);
    let raw = closed(ReservedKind::Figure, "<svg");

    let resolved = resolved(&raw, &fake).await;

    match resolved {
        Resolved::Block {
            block: ValidBlock::Figure { svg },
            ..
        } => {
            let found = forbidden_in(svg.as_str());
            assert!(found.is_empty(), "{found:?}\n{}", svg.as_str());
        }
        Resolved::Fallback { .. } => {}
        other => panic!("a figure resolved to {other:?}"),
    }
}

#[test]
fn a_figure_inside_a_catalog_visual_never_carries_anything_forbidden() {
    let hostile = read(&manifest("tests/fixtures/svg/hostile/event-attributes.svg"));
    let spec = json!({
        "root": "drawing",
        "elements": {
            "drawing": {
                "type": "figure",
                "props": { "svg": hostile, "caption": "A drawing" },
                "children": []
            }
        }
    });

    let block = validate(&closed(ReservedKind::Visual, &spec.to_string()), &ladder());

    if let Ok(ValidBlock::Visual { spec }) = block {
        let svg = spec["elements"]["drawing"]["props"]["svg"]
            .as_str()
            .expect("the figure keeps an svg");
        let found = forbidden_in(svg);
        assert!(found.is_empty(), "{found:?}\n{svg}");
    }
}

#[test]
fn a_mermaid_diagram_inside_a_catalog_visual_follows_the_mermaid_rules() {
    let spec = json!({
        "root": "flow",
        "elements": {
            "flow": {
                "type": "mermaid-diagram",
                "props": {
                    "source": "flowchart LR\n  A --> B\n  click A href \"https://evil.example\"",
                    "caption": "A flow"
                },
                "children": []
            }
        }
    });

    assert_invalid(
        ReservedKind::Visual,
        &spec.to_string(),
        "a click line in a visual",
    );
}

#[test]
fn a_record_block_is_a_programming_error_and_never_a_fallback() {
    assert_eq!(
        RawBlock::closed(ReservedKind::Record, "{}", None),
        Err(RecordBlock)
    );
    assert_eq!(
        RawBlock::unclosed(ReservedKind::Record, "{", None),
        Err(RecordBlock)
    );
}

// --- resolve: one repair, then the fallback ---------------------------------

/// For each kind: an invalid payload, a valid repair, and the block the repair
/// becomes.
fn repair_cases() -> Vec<(ReservedKind, String, String, ValidBlock)> {
    let callout = read(&manifest("tests/fixtures/catalog/valid/callout.json"));
    let callout_spec: Value = serde_json::from_str(&callout).unwrap();
    vec![
        (
            ReservedKind::Visual,
            read(&manifest(
                "tests/fixtures/catalog/invalid/missing-prop.json",
            )),
            callout,
            ValidBlock::Visual { spec: callout_spec },
        ),
        (
            ReservedKind::Figure,
            read(&manifest("tests/fixtures/svg/hostile/not-svg.svg")),
            SIMPLE_SVG.to_owned(),
            ValidBlock::Figure {
                svg: sanitise(SIMPLE_SVG).unwrap(),
            },
        ),
        (
            ReservedKind::Mermaid,
            format!("%%{{init: {{\"securityLevel\": \"loose\"}}}}%%\n{MERMAID}"),
            MERMAID.to_owned(),
            ValidBlock::Mermaid {
                source: MERMAID.to_owned(),
            },
        ),
        (
            ReservedKind::Check,
            check_payload(vec![question_json(42)]),
            check_payload(vec![question_json(7)]),
            ValidBlock::Check {
                questions: vec![expected_first(7)],
            },
        ),
    ]
}

fn assert_one_repair(fake: &FakeProvider, payload: &str, reason: &str, case: &str) {
    let requests = fake.requests();
    assert_eq!(requests.len(), 1, "{case}: {requests:#?}");
    let request = &requests[0];
    assert_eq!(request.task, Task::RepairBlock, "{case}");
    assert_eq!(request.model.as_deref(), Some(MODEL), "{case}");
    let text = sent_text(request);
    assert!(
        text.contains(payload),
        "{case}: the repair lacks the payload: {text}"
    );
    assert!(
        text.contains(reason),
        "{case}: the repair lacks the reason: {text}"
    );
}

#[tokio::test]
async fn a_valid_block_resolves_to_itself_without_a_request() {
    let fake = provider();
    let raw = closed(ReservedKind::Mermaid, MERMAID);

    let resolved = resolved(&raw, &fake).await;

    assert_eq!(
        resolved,
        Resolved::Block {
            block: ValidBlock::Mermaid {
                source: MERMAID.to_owned()
            },
            description: Some(DESCRIPTION.to_owned()),
        }
    );
    assert!(fake.requests().is_empty(), "{:#?}", fake.requests());
}

#[tokio::test]
async fn an_invalid_block_is_repaired_once_and_a_valid_repair_is_accepted() {
    for (kind, invalid, repair, expected) in repair_cases() {
        let case = format!("{kind:?}");
        let raw = closed(kind, &invalid);
        let reason = validate(&raw, &ladder()).expect_err(&case).reason;
        let fake = provider().answer(Task::RepairBlock, [ok(&repair)]);

        let resolved = resolved(&raw, &fake).await;

        assert_eq!(
            resolved,
            Resolved::Block {
                block: expected,
                description: Some(DESCRIPTION.to_owned()),
            },
            "{case}"
        );
        assert_one_repair(&fake, &invalid, &reason, &case);
    }
}

#[tokio::test]
async fn an_invalid_repair_yields_the_fallback_with_the_description_and_no_second_repair() {
    for (kind, invalid, _, _) in repair_cases() {
        let case = format!("{kind:?}");
        let raw = closed(kind, &invalid);
        let reason = validate(&raw, &ladder()).expect_err(&case).reason;
        let fake = provider().answer(Task::RepairBlock, [ok(&invalid), ok(&invalid)]);

        let resolved = resolved(&raw, &fake).await;

        assert_eq!(
            resolved,
            Resolved::Fallback {
                description: DESCRIPTION.to_owned()
            },
            "{case}"
        );
        assert_one_repair(&fake, &invalid, &reason, &case);
    }
}

#[tokio::test]
async fn a_failed_repair_request_yields_the_fallback() {
    let raw = closed(ReservedKind::Check, &check_payload(vec![question_json(42)]));
    let fake = provider().answer(Task::RepairBlock, [Err(ProviderError::Offline)]);

    let resolved = resolved(&raw, &fake).await;

    assert_eq!(
        resolved,
        Resolved::Fallback {
            description: DESCRIPTION.to_owned()
        }
    );
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_block_without_a_description_falls_back_to_a_neutral_sentence() {
    let raw = RawBlock::closed(ReservedKind::Mermaid, "", None).unwrap();
    let fake = provider().answer(Task::RepairBlock, [ok("")]);

    let resolved = resolved(&raw, &fake).await;

    assert_eq!(
        resolved,
        Resolved::Fallback {
            description: NEUTRAL_DESCRIPTION.to_owned()
        }
    );
    assert!(!NEUTRAL_DESCRIPTION.trim().is_empty());
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn an_unclosed_block_is_not_repaired() {
    let raw = RawBlock::unclosed(
        ReservedKind::Visual,
        "{\"root\": \"flow\", \"elem",
        Some(DESCRIPTION.to_owned()),
    )
    .unwrap();
    let fake = provider().answer(Task::RepairBlock, [ok(MERMAID)]);

    let resolved = resolved(&raw, &fake).await;

    assert_eq!(
        resolved,
        Resolved::Fallback {
            description: DESCRIPTION.to_owned()
        }
    );
    assert!(fake.requests().is_empty(), "{:#?}", fake.requests());
}

#[tokio::test]
async fn no_repair_is_requested_once_the_generation_was_stopped() {
    let raw = closed(ReservedKind::Mermaid, "");
    let fake = provider().answer(Task::RepairBlock, [ok(MERMAID)]);

    let resolved = resolve(&raw, &ladder(), &fake, Some(MODEL), &AtomicBool::new(true)).await;

    assert_eq!(
        resolved,
        Resolved::Fallback {
            description: DESCRIPTION.to_owned()
        }
    );
    assert!(fake.requests().is_empty(), "{:#?}", fake.requests());
}

// --- What the interface sees -------------------------------------------------

#[tokio::test]
async fn resolved_blocks_show_without_answer_keys() {
    let raw = closed(
        ReservedKind::Check,
        &check_payload(vec![question_json(7), second_question_json()]),
    );
    let resolved = resolved(&raw, &provider()).await;

    assert_eq!(
        resolved.content(),
        BlockContent::Check {
            questions: vec![
                CheckQuestion {
                    index: 0,
                    stem: "A young, warm oceanic plate meets an old, cold one. Which sinks?"
                        .to_owned(),
                    options: vec![
                        "The old, cold plate".to_owned(),
                        "The young, warm plate".to_owned(),
                        "Neither: they build mountains".to_owned(),
                        "They slide past each other".to_owned(),
                    ],
                    answer: None,
                },
                CheckQuestion {
                    index: 1,
                    stem: "What is a tectonic plate made of?".to_owned(),
                    options: vec![
                        "The crust alone".to_owned(),
                        "Crust and the rigid upper mantle".to_owned(),
                        "The asthenosphere".to_owned(),
                        "The outer core".to_owned(),
                    ],
                    answer: None,
                },
            ]
        }
    );
    assert_eq!(
        Resolved::Fallback {
            description: DESCRIPTION.to_owned()
        }
        .content(),
        BlockContent::Fallback {
            description: DESCRIPTION.to_owned()
        }
    );
    assert_eq!(
        Resolved::Block {
            block: ValidBlock::Mermaid {
                source: MERMAID.to_owned()
            },
            description: Some(DESCRIPTION.to_owned()),
        }
        .content(),
        BlockContent::Mermaid {
            source: MERMAID.to_owned(),
            description: Some(DESCRIPTION.to_owned()),
        }
    );
}
