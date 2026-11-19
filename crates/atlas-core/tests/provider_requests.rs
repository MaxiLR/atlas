//! The request shape of the plan route: the Responses body, its headers and
//! endpoint, and the model list, built and parsed without any transport.

use atlas_core::auth::AccessToken;
use atlas_core::protocol::Model;
use atlas_core::provider::chatgpt::{
    build_responses_body, parse_models, request_headers, responses_url,
};
use atlas_core::provider::{OutputSchema, ProviderRequest, Task};
use serde_json::{Map, Value, json};
use url::Url;

const PROMPT: &str = "PROMPT-7f3a: you write one section of a course.";
const CONTEXT: &str = "CONTEXT-91c2: the candidate knows linear algebra.";
const DEFAULT_MODEL: &str = "gpt-default";

/// The fields the plan route rejects (preview limitations), plus reasoning.
const REJECTED: &[&str] = &[
    "background",
    "conversation",
    "max_output_tokens",
    "max_tool_calls",
    "metadata",
    "moderation",
    "multi_agent",
    "prompt",
    "prompt_cache_retention",
    "safety_identifier",
    "temperature",
    "top_logprobs",
    "top_p",
    "truncation",
    "user",
    "previous_response_id",
    "reasoning",
    "effort",
];

fn schema() -> OutputSchema {
    OutputSchema {
        name: "ladder".to_owned(),
        schema: json!({
            "type": "object",
            "properties": { "rungs": { "type": "array", "items": { "type": "string" } } },
            "required": ["rungs"],
            "additionalProperties": false
        }),
    }
}

fn request(web_search: bool, with_schema: bool) -> ProviderRequest {
    ProviderRequest {
        task: Task::DraftLadder,
        model: Some("gpt-chosen".to_owned()),
        instructions: PROMPT.to_owned(),
        input: CONTEXT.to_owned(),
        web_search,
        schema: with_schema.then(schema),
    }
}

fn combinations() -> Vec<(bool, bool)> {
    vec![(false, false), (false, true), (true, false), (true, true)]
}

/// Every object key in `value`, at every depth, except inside the caller's own
/// JSON schema, whose property names are the caller's data.
fn keys(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}/{key}");
                found.push(key.clone());
                if child_path != "/text/format/schema" {
                    keys(child, &child_path, found);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                keys(item, &format!("{path}/[]"), found);
            }
        }
        _ => {}
    }
}

fn object(value: &Value) -> &Map<String, Value> {
    value.as_object().expect("the body is a JSON object")
}

#[test]
fn every_combination_has_the_required_fields_and_no_rejected_ones() {
    for (web_search, with_schema) in combinations() {
        let body = build_responses_body(&request(web_search, with_schema), DEFAULT_MODEL);
        let case = format!("search {web_search}, schema {with_schema}: {body}");
        let fields = object(&body);

        assert_eq!(fields.get("model"), Some(&json!("gpt-chosen")), "{case}");
        assert_eq!(fields.get("instructions"), Some(&json!(PROMPT)), "{case}");
        assert_eq!(fields.get("store"), Some(&json!(false)), "{case}");
        assert_eq!(fields.get("stream"), Some(&json!(true)), "{case}");

        let input = fields
            .get("input")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("input is an array: {case}"));
        assert!(!input.is_empty(), "{case}");
        assert!(
            input.iter().any(|item| item.to_string().contains(CONTEXT)),
            "the full context is in input: {case}"
        );

        let mut found = Vec::new();
        keys(&body, "", &mut found);
        for rejected in REJECTED {
            assert!(
                !found.iter().any(|key| key == rejected),
                "{rejected} must not appear: {case}"
            );
        }
    }
}

#[test]
fn the_web_search_tool_is_sent_only_when_search_is_on() {
    for (web_search, with_schema) in combinations() {
        let body = build_responses_body(&request(web_search, with_schema), DEFAULT_MODEL);
        let tools = object(&body).get("tools");
        if web_search {
            assert_eq!(
                tools,
                Some(&json!([{ "type": "web_search" }])),
                "schema {with_schema}: {body}"
            );
        } else {
            assert_eq!(tools, None, "schema {with_schema}: {body}");
        }
    }
}

#[test]
fn structured_output_is_a_json_schema_text_format_only_when_a_schema_is_given() {
    for (web_search, with_schema) in combinations() {
        let body = build_responses_body(&request(web_search, with_schema), DEFAULT_MODEL);
        let text = object(&body).get("text");
        if with_schema {
            let format = text
                .and_then(|text| text.get("format"))
                .unwrap_or_else(|| panic!("text.format is present: {body}"));
            assert_eq!(format.get("type"), Some(&json!("json_schema")), "{body}");
            assert_eq!(format.get("name"), Some(&json!("ladder")), "{body}");
            assert_eq!(format.get("schema"), Some(&schema().schema), "{body}");
        } else {
            assert_eq!(text, None, "search {web_search}: {body}");
        }
    }
}

#[test]
fn the_prompt_appears_only_in_instructions_and_no_input_item_is_a_system_message() {
    for (web_search, with_schema) in combinations() {
        let mut body = build_responses_body(&request(web_search, with_schema), DEFAULT_MODEL);
        let input = body["input"].as_array().expect("input is an array").clone();
        for item in &input {
            assert_ne!(item.get("role"), Some(&json!("system")), "{item}");
        }

        body.as_object_mut()
            .expect("the body is a JSON object")
            .remove("instructions");
        assert!(
            !body.to_string().contains("PROMPT-7f3a"),
            "the prompt leaks outside instructions: {body}"
        );
    }
}

#[test]
fn the_default_model_is_used_when_the_request_names_none() {
    let mut without_model = request(false, false);
    without_model.model = None;
    let body = build_responses_body(&without_model, DEFAULT_MODEL);
    assert_eq!(body["model"], json!(DEFAULT_MODEL));
}

#[test]
fn the_headers_carry_the_bearer_token_and_a_json_content_type() {
    let headers = request_headers(&AccessToken::new("token-abc".to_owned()));
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer token-abc")
    );
    assert_eq!(
        headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
}

#[test]
fn the_endpoint_is_responses_under_the_injected_api_base() {
    let cases = [
        (
            "https://api.openai.com/v1",
            "https://api.openai.com/v1/responses",
        ),
        (
            "https://api.openai.com/v1/",
            "https://api.openai.com/v1/responses",
        ),
        (
            "http://127.0.0.1:4100/api",
            "http://127.0.0.1:4100/api/responses",
        ),
    ];
    for (base, expected) in cases {
        let base = Url::parse(base).expect("a valid base");
        assert_eq!(responses_url(&base).as_str(), expected);
    }
}

fn model(slug: &str, display_name: &str) -> Model {
    Model {
        slug: slug.to_owned(),
        display_name: display_name.to_owned(),
    }
}

#[test]
fn the_model_list_keeps_only_listed_models_in_the_reported_order() {
    let body = json!({
        "models": [
            { "slug": "zeta-2", "display_name": "Zeta 2", "visibility": "list" },
            { "slug": "internal-1", "display_name": "Internal", "visibility": "hide" },
            { "slug": "alpha-9", "display_name": "Alpha 9", "visibility": "list" },
            { "slug": "legacy", "display_name": "Legacy", "visibility": "none" },
            { "slug": "mu-4", "display_name": "Mu 4", "visibility": "list" }
        ]
    })
    .to_string();

    assert_eq!(
        parse_models(&body),
        Ok(vec![
            model("zeta-2", "Zeta 2"),
            model("alpha-9", "Alpha 9"),
            model("mu-4", "Mu 4"),
        ])
    );
}

#[test]
fn an_empty_or_malformed_model_list_is_an_error() {
    let bodies = [
        String::new(),
        "not json".to_owned(),
        "[]".to_owned(),
        json!({}).to_string(),
        json!({ "models": null }).to_string(),
        json!({ "models": "gpt" }).to_string(),
        json!({ "models": [] }).to_string(),
        json!({ "models": [{ "slug": "hidden", "display_name": "Hidden", "visibility": "hide" }] })
            .to_string(),
        json!({ "models": [{ "display_name": "No slug", "visibility": "list" }] }).to_string(),
        json!({ "models": [{ "slug": "no-name", "visibility": "list" }] }).to_string(),
        json!({ "models": [{ "slug": 7, "display_name": "Seven", "visibility": "list" }] })
            .to_string(),
    ];
    for body in bodies {
        assert!(parse_models(&body).is_err(), "{body:?} is accepted");
    }
}
