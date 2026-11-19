//! The structured-output runner against a scripted fake provider: the requests
//! it makes for a valid answer, a route that refuses structured output, an
//! answer that fails the schema or the domain check, and provider errors.

use std::sync::Arc;

use atlas_core::harness::prompts::{self, Prompt};
use atlas_core::harness::structured::{StructuredError, StructuredRunner, StructuredTask};
use atlas_core::provider::{
    FakeProvider, FakeScript, OutputSchema, ProviderError, ProviderRequest, Task,
};
use serde::Deserialize;
use serde_json::json;

/// Words that appear in the schema only, so a prompt that holds them holds
/// the schema.
const SCHEMA_DESCRIPTION: &str = "The chapter titles, broad first, in reading order";
const DOMAIN_REASON: &str = "an index needs at least two chapters";
const VALID: &str = r#"{"chapters": ["Foundations", "Core ideas"]}"#;
const MODEL: &str = "gpt-test";
const CONTEXT: &str = "Topic: thermodynamics";

#[derive(Debug, PartialEq, Deserialize)]
struct Chapters {
    chapters: Vec<String>,
}

/// A task shaped like build index: a list of chapter titles, at least two.
struct ChaptersTask;

impl StructuredTask for ChaptersTask {
    type Output = Chapters;

    fn task(&self) -> Task {
        Task::BuildIndex
    }

    fn prompt(&self) -> Prompt {
        prompts::BUILD_INDEX
    }

    fn schema(&self) -> OutputSchema {
        schema()
    }

    fn check(&self, value: &Chapters) -> Result<(), Vec<String>> {
        if value.chapters.len() < 2 {
            Err(vec![DOMAIN_REASON.to_owned()])
        } else {
            Ok(())
        }
    }
}

fn schema() -> OutputSchema {
    OutputSchema {
        name: "chapters".to_owned(),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["chapters"],
            "properties": {
                "chapters": {
                    "type": "array",
                    "description": SCHEMA_DESCRIPTION,
                    "items": { "type": "string" }
                }
            }
        }),
    }
}

fn valid() -> Chapters {
    Chapters {
        chapters: vec!["Foundations".to_owned(), "Core ideas".to_owned()],
    }
}

fn provider() -> FakeProvider {
    FakeProvider::new(FakeScript::sample())
}

fn ok(text: &str) -> Result<String, ProviderError> {
    Ok(text.to_owned())
}

fn structured_output_refused() -> ProviderError {
    ProviderError::Unsupported {
        param: Some("text.format".to_owned()),
        request_id: Some("req_refused".to_owned()),
    }
}

/// Everything a request sends the model as text.
fn sent_text(request: &ProviderRequest) -> String {
    format!("{}\n{}", request.instructions, request.input)
}

async fn run(runner: &StructuredRunner, task: &ChaptersTask) -> Result<Chapters, StructuredError> {
    runner.run(task, Some(MODEL), CONTEXT).await
}

#[tokio::test]
async fn a_valid_answer_takes_one_request_carrying_the_schema_as_structured_output() {
    let fake = Arc::new(provider().answer(Task::BuildIndex, [ok(VALID)]));
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(value, Ok(valid()));
    let requests = fake.requests();
    assert_eq!(requests.len(), 1, "{requests:#?}");
    let request = &requests[0];
    assert_eq!(request.task, Task::BuildIndex);
    assert_eq!(request.schema, Some(schema()));
    assert_eq!(request.model.as_deref(), Some(MODEL));
    assert!(request.instructions.contains(prompts::BUILD_INDEX.text()));
    assert!(request.input.contains(CONTEXT));
}

#[tokio::test]
async fn a_refused_structured_output_is_asked_again_with_the_schema_in_the_prompt_and_remembered() {
    let fake = Arc::new(provider().answer(
        Task::BuildIndex,
        [Err(structured_output_refused()), ok(VALID), ok(VALID)],
    ));
    let runner = StructuredRunner::new(fake.clone());

    let first = run(&runner, &ChaptersTask).await;

    assert_eq!(first, Ok(valid()));
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "{requests:#?}");
    assert_eq!(requests[0].schema, Some(schema()));
    assert_eq!(requests[1].task, Task::BuildIndex);
    assert_eq!(requests[1].schema, None);
    assert!(
        requests[1].instructions.contains(SCHEMA_DESCRIPTION),
        "the prompt holds no schema: {}",
        requests[1].instructions
    );
    assert!(
        requests[1]
            .instructions
            .contains(prompts::BUILD_INDEX.text())
    );
    assert!(requests[1].input.contains(CONTEXT));

    let second = run(&runner, &ChaptersTask).await;

    assert_eq!(second, Ok(valid()));
    let requests = fake.requests();
    assert_eq!(requests.len(), 3, "{requests:#?}");
    assert_eq!(requests[2].schema, None);
    assert!(requests[2].instructions.contains(SCHEMA_DESCRIPTION));
}

#[tokio::test]
async fn an_answer_that_fails_the_schema_is_repaired_once_with_the_answer_and_its_errors() {
    let invalid = r#"{"chapters": "Foundations"}"#;
    let fake = Arc::new(
        provider()
            .answer(Task::BuildIndex, [ok(invalid)])
            .answer(Task::RepairStructured, [ok(VALID)]),
    );
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(value, Ok(valid()));
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "{requests:#?}");
    let repair = &requests[1];
    assert_eq!(repair.task, Task::RepairStructured);
    let text = sent_text(repair);
    assert!(
        text.contains(invalid),
        "the repair lacks the answer: {text}"
    );
    assert!(
        text.contains(r#""Foundations" is not of type "array""#),
        "the repair lacks the validation error: {text}"
    );
    assert!(
        text.contains(SCHEMA_DESCRIPTION),
        "the repair lacks the schema: {text}"
    );
}

#[tokio::test]
async fn an_answer_that_fails_twice_ends_in_a_typed_failure_after_two_requests() {
    let fake = Arc::new(
        provider()
            .answer(Task::BuildIndex, [ok("Here are the chapters: Foundations")])
            .answer(Task::RepairStructured, [ok(r#"{"chapters": 3}"#)]),
    );
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    match value {
        Err(StructuredError::Invalid { reasons }) => {
            assert!(!reasons.is_empty(), "a failure names what was wrong");
        }
        other => panic!("expected an invalid answer, got {other:?}"),
    }
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "{requests:#?}");
}

#[tokio::test]
async fn a_failed_domain_check_is_repaired_the_same_way() {
    let wrong = r#"{"chapters": ["Everything"]}"#;
    let fake = Arc::new(
        provider()
            .answer(Task::BuildIndex, [ok(wrong)])
            .answer(Task::RepairStructured, [ok(VALID)]),
    );
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(value, Ok(valid()));
    let requests = fake.requests();
    assert_eq!(requests.len(), 2, "{requests:#?}");
    let repair = &requests[1];
    assert_eq!(repair.task, Task::RepairStructured);
    let text = sent_text(repair);
    assert!(text.contains(wrong), "the repair lacks the answer: {text}");
    assert!(
        text.contains(DOMAIN_REASON),
        "the repair lacks the reason: {text}"
    );
}

#[tokio::test]
async fn a_failed_domain_check_after_repair_ends_in_a_typed_failure() {
    let wrong = r#"{"chapters": ["Everything"]}"#;
    let fake = Arc::new(
        provider()
            .answer(Task::BuildIndex, [ok(wrong)])
            .answer(Task::RepairStructured, [ok(wrong)]),
    );
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(
        value,
        Err(StructuredError::Invalid {
            reasons: vec![DOMAIN_REASON.to_owned()]
        })
    );
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn a_plan_limit_is_returned_as_is_without_a_repair() {
    let limit = ProviderError::PlanLimit {
        resets_at: None,
        request_id: Some("req_limit".to_owned()),
    };
    let fake = Arc::new(provider().answer(Task::BuildIndex, [Err(limit.clone())]));
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(value, Err(StructuredError::Provider(limit)));
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_refused_capability_other_than_the_output_format_passes_through() {
    let refused = ProviderError::Unsupported {
        param: Some("tools".to_owned()),
        request_id: Some("req_tools".to_owned()),
    };
    let fake = Arc::new(provider().answer(Task::BuildIndex, [Err(refused.clone())]));
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(value, Err(StructuredError::Provider(refused)));
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_repair_after_a_refusal_also_carries_the_schema_in_the_prompt() {
    let invalid = r#"{"chapters": "Foundations"}"#;
    let fake = Arc::new(
        provider()
            .answer(
                Task::BuildIndex,
                [Err(structured_output_refused()), ok(invalid)],
            )
            .answer(Task::RepairStructured, [ok(VALID)]),
    );
    let runner = StructuredRunner::new(fake.clone());

    let value = run(&runner, &ChaptersTask).await;

    assert_eq!(value, Ok(valid()));
    let requests = fake.requests();
    assert_eq!(requests.len(), 3, "{requests:#?}");
    let repair = &requests[2];
    assert_eq!(repair.task, Task::RepairStructured);
    assert_eq!(repair.schema, None);
    assert!(sent_text(repair).contains(SCHEMA_DESCRIPTION));
}
