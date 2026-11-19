//! Tasks whose output is data (ladder, question, index, deep-dive proposals):
//! structured output against a JSON schema, or the schema in the prompt when
//! the route refuses it, with one repair request.

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use jsonschema::Validator;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::harness::prompts::{self, Prompt};
use crate::provider::{
    ModelProvider, OutputSchema, ProviderError, ProviderRequest, Task, complete_text,
};

/// Why a structured task ended without a value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StructuredError {
    /// The answer failed the schema or the domain check, repair included.
    #[error("the answer is invalid: {}", reasons.join("; "))]
    Invalid { reasons: Vec<String> },
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// A task whose answer is data: its prompt, the JSON schema the answer must
/// follow, and the domain check the parsed value must pass.
pub trait StructuredTask: Sync {
    type Output: DeserializeOwned;

    /// The harness task its requests belong to.
    fn task(&self) -> Task;

    /// The prompt, sent as the request's instructions.
    fn prompt(&self) -> Prompt;

    /// The JSON schema of the answer.
    fn schema(&self) -> OutputSchema;

    /// What is wrong with a value that follows the schema, if anything.
    fn check(&self, value: &Self::Output) -> Result<(), Vec<String>>;
}

/// Runs structured tasks for one session. A route that refuses structured
/// output is remembered for the runner's life, so later tasks put the schema
/// in the prompt at once.
pub struct StructuredRunner {
    provider: Arc<dyn ModelProvider>,
    schema_in_prompt: AtomicBool,
}

impl StructuredRunner {
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        Self {
            provider,
            schema_in_prompt: AtomicBool::new(false),
        }
    }

    /// Runs `task` on `context` (the request's input), on `model` or the
    /// provider's default, and returns the checked value. An answer that
    /// fails the schema or the domain check gets one repair request; provider
    /// errors other than a refused output format are returned unchanged.
    pub async fn run<T: StructuredTask>(
        &self,
        task: &T,
        model: Option<&str>,
        context: &str,
    ) -> Result<T::Output, StructuredError> {
        let schema = task.schema();
        let validator = jsonschema::validator_for(&schema.schema)
            .unwrap_or_else(|error| panic!("the schema {} is invalid: {error}", schema.name));
        let instructions = task.prompt().text();

        let answer = self
            .ask(
                task.task(),
                model,
                instructions.to_owned(),
                context.to_owned(),
                &schema,
                false,
            )
            .await?;
        let reasons = match judge(task, &validator, &answer) {
            Ok(value) => return Ok(value),
            Err(reasons) => reasons,
        };

        let repair = repair_input(instructions, context, &answer, &reasons, &schema);
        let answer = self
            .ask(
                Task::RepairStructured,
                model,
                prompts::REPAIR_STRUCTURED.text().to_owned(),
                repair,
                &schema,
                true,
            )
            .await?;
        judge(task, &validator, &answer).map_err(|reasons| StructuredError::Invalid { reasons })
    }

    /// One answer: asked for as structured output, unless the route refuses
    /// it, in which case the core validates alone and the schema is written
    /// into the instructions, unless `input` holds it already.
    async fn ask(
        &self,
        task: Task,
        model: Option<&str>,
        instructions: String,
        input: String,
        schema: &OutputSchema,
        schema_in_input: bool,
    ) -> Result<String, ProviderError> {
        if !self.schema_in_prompt.load(Ordering::Relaxed) {
            let request = ProviderRequest {
                model: model.map(str::to_owned),
                schema: Some(schema.clone()),
                ..ProviderRequest::new(task, instructions.clone(), input.clone())
            };
            match complete_text(self.provider.as_ref(), request, |_| {}).await {
                Err(error) if refuses_output_format(&error) => {
                    self.schema_in_prompt.store(true, Ordering::Relaxed);
                }
                answer => return answer,
            }
        }
        let instructions = if schema_in_input {
            instructions
        } else {
            format!(
                "{instructions}\n\nAnswer with JSON only, with no other text, following this \
                 JSON schema:\n\n{}",
                pretty(&schema.schema)
            )
        };
        let request = ProviderRequest {
            model: model.map(str::to_owned),
            ..ProviderRequest::new(task, instructions, input)
        };
        complete_text(self.provider.as_ref(), request, |_| {}).await
    }
}

/// Whether the route refused the request's output format (`text.format`).
fn refuses_output_format(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::Unsupported { param: Some(param), .. }
            if param == "text" || param.starts_with("text.")
    )
}

/// The value `answer` holds, or everything wrong with it: not JSON, the
/// schema's errors, or the domain check's.
fn judge<T: StructuredTask>(
    task: &T,
    validator: &Validator,
    answer: &str,
) -> Result<T::Output, Vec<String>> {
    let json: Value = serde_json::from_str(strip_fence(answer))
        .map_err(|error| vec![format!("the answer is not JSON: {error}")])?;
    let errors: Vec<String> = validator
        .iter_errors(&json)
        .map(|error| {
            let path = error.instance_path().to_string();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("at {path}: {error}")
            }
        })
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let value = serde_json::from_value(json).map_err(|error| {
        vec![format!(
            "the answer does not have the expected shape: {error}"
        )]
    })?;
    task.check(&value)?;
    Ok(value)
}

/// The answer without a Markdown code fence around it.
fn strip_fence(answer: &str) -> &str {
    let answer = answer.trim();
    answer
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
        .map(|inner| {
            let inner = inner.strip_prefix("json").unwrap_or(inner);
            inner.trim()
        })
        .unwrap_or(answer)
}

/// The repair request's input: the task, the answer, what was wrong and the
/// schema.
fn repair_input(
    instructions: &str,
    context: &str,
    answer: &str,
    reasons: &[String],
    schema: &OutputSchema,
) -> String {
    let mut input = format!(
        "## The task's instructions\n\n{instructions}\n\n## The task's input\n\n{context}\n\n\
         ## Your answer\n\n{answer}\n\n## What was wrong\n\n"
    );
    for reason in reasons {
        let _ = writeln!(input, "- {reason}");
    }
    let _ = write!(input, "\n## The JSON schema\n\n{}", pretty(&schema.schema));
    input
}

fn pretty(schema: &Value) -> String {
    serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string())
}
