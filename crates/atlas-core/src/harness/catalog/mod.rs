//! The catalog of visuals. Its prompt and its JSON schema are generated from
//! the interface's TypeScript catalog (`ui/src/catalog/`) by
//! `ui/scripts/gen-catalog.ts` into `prompts/generated/`, and embedded here,
//! so the model's instructions, this validation and the interface's rendering
//! cannot drift apart.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

const PROMPT_SOURCE: &str = include_str!("../../../../../prompts/generated/catalog.prompt.md");
const SCHEMA_SOURCE: &str = include_str!("../../../../../prompts/generated/catalog.schema.json");

static SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(SCHEMA_SOURCE)
        .unwrap_or_else(|error| panic!("the catalog's schema is not JSON: {error}"))
});

static VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    jsonschema::validator_for(&SCHEMA)
        .unwrap_or_else(|error| panic!("the catalog's schema is invalid: {error}"))
});

/// Why a visual spec was refused, one reason per fault.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{}", reasons.join("; "))]
pub struct InvalidSpec {
    pub reasons: Vec<String>,
}

/// The catalog's instructions for the model, without front matter.
pub fn prompt() -> &'static str {
    split_front_matter().1.trim()
}

/// The catalog's JSON schema, as generated.
pub fn schema() -> &'static Value {
    &SCHEMA
}

/// The version named in the prompt's front matter.
pub fn version() -> u32 {
    split_front_matter()
        .0
        .lines()
        .find_map(|line| line.strip_prefix("version:"))
        .and_then(|value| value.trim().parse().ok())
        .expect("the catalog's prompt names no version")
}

/// The names of the catalog's entries, in catalog order.
pub fn component_names() -> Vec<&'static str> {
    SCHEMA["$defs"]["element"]["properties"]["type"]["enum"]
        .as_array()
        .expect("the catalog's schema lists its entries")
        .iter()
        .map(|name| name.as_str().expect("an entry's name is a string"))
        .collect()
}

/// Checks a visual spec against the catalog: its shape and every prop, then
/// that its root names its one element, which the schema cannot say.
pub fn validate(spec: &Value) -> Result<(), InvalidSpec> {
    let mut reasons: Vec<String> = VALIDATOR
        .iter_errors(spec)
        .map(|error| {
            let path = error.instance_path().to_string();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("at {path}: {error}")
            }
        })
        .collect();
    if reasons.is_empty()
        && let Some(root) = spec["root"].as_str()
        && spec["elements"].get(root).is_none()
    {
        reasons.push(format!("the root \"{root}\" names no element"));
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(InvalidSpec { reasons })
    }
}

fn split_front_matter() -> (&'static str, &'static str) {
    PROMPT_SOURCE
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .expect("the catalog's prompt has no front matter")
}
