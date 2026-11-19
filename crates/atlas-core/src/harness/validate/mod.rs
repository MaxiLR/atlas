//! Validation of the blocks a section writes (catalog visual, figure, Mermaid,
//! check questions), one repair request, then the text fallback.
//!
//! This is the boundary between the model's output and what a candidate
//! sees. Each shown block the splitter reports ([`crate::harness::splitter`])
//! is checked by [`validate`]:
//!
//! - `atlas-visual`: JSON that passes the catalog ([`catalog::validate`]). Its
//!   one element follows the rules of its own kind below: a `figure` entry's
//!   SVG is replaced by its sanitised form, a `mermaid-diagram` entry's source
//!   follows the Mermaid rules, a `check-question` entry names a rung on the
//!   ladder.
//! - `atlas-figure`: an SVG that passes the sanitiser
//!   ([`crate::sanitise::sanitise`]); the block carries the sanitised SVG.
//! - `mermaid`: Mermaid source that is not empty, at most
//!   [`MAX_MERMAID_BYTES`] long, with no directive (`%%{init: …}%%`), no front
//!   matter, and no `click`, `link`, `links` or `callback` statement.
//! - `atlas-check`: one JSON object, `{"questions": [ … ]}`, holding one or
//!   two questions, each shaped as the catalog's `check-question` entry: a
//!   rung, a stem, four options with their misconceptions, one key and an
//!   explanation. Each names a rung on the ladder.
//!
//! [`resolve`] turns a block into what is shown: valid, the block; invalid,
//! one repair request ([`prompts::REPAIR_BLOCK`]) with the payload, the reason
//! and the format, whose answer is validated again; still invalid, or the
//! repair failed, the text fallback with the block's `alt:` description. An
//! unclosed block, or one from a stopped generation, is not repaired.
//!
//! The section record is not a shown block: `harness::record` validates and
//! repairs it, and [`RawBlock`] refuses to hold it.

use std::fmt::Write as _;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use jsonschema::Validator;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::ladder::{Ladder, RungId};
use crate::domain::questions::KeyedCheckQuestion;
use crate::harness::catalog;
use crate::harness::prompts;
use crate::harness::splitter::ReservedKind;
use crate::protocol::{BlockContent, CheckQuestion};
use crate::provider::{ModelProvider, ProviderRequest, Task, complete_text};
use crate::sanitise::{SanitisedSvg, sanitise};

/// The largest Mermaid source accepted, in bytes.
pub const MAX_MERMAID_BYTES: usize = 16 * 1024;

/// The most check questions one block holds.
pub const MAX_QUESTIONS: usize = 2;

/// What a fallback shows when its block came with no description.
pub const NEUTRAL_DESCRIPTION: &str = "This part of the section could not be shown.";

/// Mermaid statements that make a diagram interactive.
const INTERACTIONS: [&str; 4] = ["click", "link", "links", "callback"];

/// The schema of an `atlas-check` payload, built around the catalog's
/// `check-question` entry so the two cannot drift.
static CHECK_SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    let schema = catalog::schema();
    json!({
        "$schema": schema["$schema"],
        "type": "object",
        "required": ["questions"],
        "additionalProperties": false,
        "properties": {
            "questions": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_QUESTIONS,
                "items": schema["$defs"]["props"]["check-question"],
            }
        }
    })
});

static CHECK_VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    jsonschema::validator_for(&CHECK_SCHEMA)
        .unwrap_or_else(|error| panic!("the check questions' schema is invalid: {error}"))
});

const FIGURE_FORMAT: &str = "One static <svg> element. No script, no foreignObject, no event \
    attributes, no external references (href, image, use), no url() in styles except to a \
    definition in the same figure; colours only as currentColor or the theme's variables \
    (var(--atlas-…)). It must draw something.";

const CHECK_FORMAT: &str = "One JSON object, {\"questions\": [ … ]}, holding one or two \
    questions. Each has a rung (the id of the ladder rung it tests, from the list below), a \
    stem, exactly four options, each with its text and its misconception (null for the right \
    option, one sentence for every wrong one), one key (the right option, counted from 0) and \
    an explanation. Each question follows this JSON schema:";

/// Why a block's payload was refused, quoted in the repair request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct BlockFault {
    pub reason: String,
}

impl BlockFault {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// The section record was handed to this module: a programming error, since
/// `harness::record` validates, repairs and replaces the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the section record is not a shown block: harness::record resolves it")]
pub struct RecordBlock;

/// A shown block as the splitter reported it: its kind, its raw payload, its
/// `alt:` description, and whether its fence closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBlock {
    kind: ReservedKind,
    payload: String,
    description: Option<String>,
    closed: bool,
}

impl RawBlock {
    /// A block whose fence closed.
    pub fn closed(
        kind: ReservedKind,
        payload: impl Into<String>,
        description: Option<String>,
    ) -> Result<Self, RecordBlock> {
        Self::new(kind, payload.into(), description, true)
    }

    /// A block still open when the stream ended.
    pub fn unclosed(
        kind: ReservedKind,
        payload: impl Into<String>,
        description: Option<String>,
    ) -> Result<Self, RecordBlock> {
        Self::new(kind, payload.into(), description, false)
    }

    fn new(
        kind: ReservedKind,
        payload: String,
        description: Option<String>,
        closed: bool,
    ) -> Result<Self, RecordBlock> {
        if kind == ReservedKind::Record {
            return Err(RecordBlock);
        }
        Ok(Self {
            kind,
            payload,
            description,
            closed,
        })
    }

    pub fn kind(&self) -> ReservedKind {
        self.kind
    }

    pub fn payload(&self) -> &str {
        &self.payload
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// What the text fallback shows: the description, or a neutral sentence.
    fn fallback(&self) -> Resolved {
        let description = self
            .description
            .as_deref()
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .unwrap_or(NEUTRAL_DESCRIPTION);
        Resolved::Fallback {
            description: description.to_owned(),
        }
    }
}

/// A block that passed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidBlock {
    /// A json-render spec that passed the catalog.
    Visual {
        spec: Value,
    },
    Figure {
        svg: SanitisedSvg,
    },
    Mermaid {
        source: String,
    },
    /// One or two check questions, with their keys.
    Check {
        questions: Vec<KeyedCheckQuestion>,
    },
}

/// What a block resolves to: valid, possibly after its repair, or the text
/// fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Block {
        block: ValidBlock,
        description: Option<String>,
    },
    Fallback {
        description: String,
    },
}

impl Resolved {
    /// The block as the interface shows it; check questions without their
    /// keys.
    pub fn content(&self) -> BlockContent {
        let (block, description) = match self {
            Self::Fallback { description } => {
                return BlockContent::Fallback {
                    description: description.clone(),
                };
            }
            Self::Block { block, description } => (block, description.clone()),
        };
        match block {
            ValidBlock::Visual { spec } => BlockContent::Visual {
                spec: spec.clone(),
                description,
            },
            ValidBlock::Figure { svg } => BlockContent::Figure {
                svg: svg.clone(),
                description,
            },
            ValidBlock::Mermaid { source } => BlockContent::Mermaid {
                source: source.clone(),
                description,
            },
            ValidBlock::Check { questions } => BlockContent::Check {
                questions: (0..)
                    .zip(questions)
                    .map(|(index, question)| CheckQuestion {
                        index,
                        stem: question.stem.clone(),
                        options: question
                            .options
                            .iter()
                            .map(|option| option.text.clone())
                            .collect(),
                        answer: None,
                    })
                    .collect(),
            },
        }
    }
}

/// Checks a block's payload against the rules of its kind.
pub fn validate(raw: &RawBlock, ladder: &Ladder) -> Result<ValidBlock, BlockFault> {
    match raw.kind {
        ReservedKind::Visual => visual(&raw.payload, ladder),
        ReservedKind::Figure => figure(&raw.payload).map(|svg| ValidBlock::Figure { svg }),
        ReservedKind::Mermaid => mermaid(&raw.payload).map(|()| ValidBlock::Mermaid {
            source: raw.payload.clone(),
        }),
        ReservedKind::Check => check(&raw.payload, ladder),
        ReservedKind::Record => unreachable!("a RawBlock never holds the section record"),
    }
}

/// Resolves a block: valid, it is the block; invalid, one repair request on
/// `model` (the provider's default when `None`), validated again; still
/// invalid, or the request failed, the text fallback. An unclosed block, or
/// one whose generation was `stopped`, gets no repair request.
pub async fn resolve(
    raw: &RawBlock,
    ladder: &Ladder,
    provider: &dyn ModelProvider,
    model: Option<&str>,
    stopped: &AtomicBool,
) -> Resolved {
    let fault = match validate(raw, ladder) {
        Ok(block) => return valid(raw, block),
        Err(fault) => fault,
    };
    if !raw.closed || stopped.load(Ordering::Acquire) {
        return raw.fallback();
    }
    let request = ProviderRequest {
        model: model.map(str::to_owned),
        ..ProviderRequest::new(
            Task::RepairBlock,
            prompts::REPAIR_BLOCK.text(),
            repair_input(raw, &fault, ladder),
        )
    };
    let Ok(answer) = complete_text(provider, request, |_| {}).await else {
        return raw.fallback();
    };
    let repaired = RawBlock {
        payload: strip_fence(&answer).to_owned(),
        ..raw.clone()
    };
    match validate(&repaired, ladder) {
        Ok(block) => valid(raw, block),
        Err(_) => raw.fallback(),
    }
}

fn valid(raw: &RawBlock, block: ValidBlock) -> Resolved {
    Resolved::Block {
        block,
        description: raw.description.clone(),
    }
}

fn visual(payload: &str, ladder: &Ladder) -> Result<ValidBlock, BlockFault> {
    let mut spec: Value = parse_json(payload)?;
    catalog::validate(&spec).map_err(|invalid| BlockFault::new(invalid.to_string()))?;
    let elements = spec["elements"]
        .as_object_mut()
        .expect("the catalog's schema requires an elements object");
    for element in elements.values_mut() {
        let kind = element["type"].as_str().unwrap_or_default().to_owned();
        let props = &mut element["props"];
        match kind.as_str() {
            "figure" => {
                let svg = props["svg"].as_str().unwrap_or_default();
                let sanitised = figure(svg)?;
                props["svg"] = Value::String(sanitised.as_str().to_owned());
            }
            "mermaid-diagram" => mermaid(props["source"].as_str().unwrap_or_default())?,
            "check-question" => {
                let rung = props["rung"].as_u64().unwrap_or_default();
                if !on_ladder(ladder, rung) {
                    return Err(BlockFault::new(format!(
                        "the question names rung {rung}, which is not on the ladder"
                    )));
                }
            }
            _ => {}
        }
    }
    Ok(ValidBlock::Visual { spec })
}

fn figure(svg: &str) -> Result<SanitisedSvg, BlockFault> {
    sanitise(svg.trim()).map_err(|rejected| BlockFault::new(rejected.to_string()))
}

fn mermaid(source: &str) -> Result<(), BlockFault> {
    if source.trim().is_empty() {
        return Err(BlockFault::new("the diagram is empty"));
    }
    if source.len() > MAX_MERMAID_BYTES {
        return Err(BlockFault::new(format!(
            "the diagram is {} bytes, more than the {MAX_MERMAID_BYTES} allowed",
            source.len()
        )));
    }
    if source.trim_start().starts_with("---") {
        return Err(BlockFault::new(
            "the diagram starts with front matter, which can configure Mermaid; leave it out",
        ));
    }
    for (number, line) in (1..).zip(source.lines()) {
        if line.contains("%%{") {
            return Err(BlockFault::new(format!(
                "line {number} holds a directive (%%{{…}}%%), such as init, which configures \
                 Mermaid; leave it out"
            )));
        }
        let interaction = line.split(';').find_map(|statement| {
            let word = statement.split_whitespace().next()?;
            INTERACTIONS
                .into_iter()
                .find(|keyword| word.eq_ignore_ascii_case(keyword))
        });
        if let Some(keyword) = interaction {
            return Err(BlockFault::new(format!(
                "line {number} is a `{keyword}` statement; a diagram takes no click or link \
                 interactions"
            )));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct CheckPayload {
    questions: Vec<KeyedCheckQuestion>,
}

fn check(payload: &str, ladder: &Ladder) -> Result<ValidBlock, BlockFault> {
    let json = parse_json(payload)?;
    let errors: Vec<String> = CHECK_VALIDATOR
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
        return Err(BlockFault::new(errors.join("; ")));
    }
    let CheckPayload { questions } = serde_json::from_value(json).map_err(|error| {
        BlockFault::new(format!(
            "the questions do not have the expected shape: {error}"
        ))
    })?;
    for (number, question) in (1..).zip(&questions) {
        let RungId(rung) = question.rung;
        if !on_ladder(ladder, rung.into()) {
            return Err(BlockFault::new(format!(
                "question {number} names rung {rung}, which is not on the ladder"
            )));
        }
    }
    Ok(ValidBlock::Check { questions })
}

fn on_ladder(ladder: &Ladder, rung: u64) -> bool {
    ladder
        .rungs
        .iter()
        .any(|candidate| u64::from(candidate.id.0) == rung)
}

fn parse_json(payload: &str) -> Result<Value, BlockFault> {
    serde_json::from_str(payload.trim())
        .map_err(|error| BlockFault::new(format!("the payload is not JSON: {error}")))
}

/// The repair request's input: the kind, the payload, what was wrong and the
/// format.
fn repair_input(raw: &RawBlock, fault: &BlockFault, ladder: &Ladder) -> String {
    let mut input = format!(
        "## The block\n\nA `{}` block of a section.\n\n## Your payload\n\n{}\n\n\
         ## What was wrong\n\n{}\n\n## The format\n\n",
        raw.kind.name(),
        raw.payload,
        fault.reason
    );
    match raw.kind {
        ReservedKind::Visual => input.push_str(catalog::prompt()),
        ReservedKind::Figure => input.push_str(FIGURE_FORMAT),
        ReservedKind::Mermaid => {
            let _ = write!(
                input,
                "Mermaid source for a flowchart, sequence or state diagram, at most \
                 {MAX_MERMAID_BYTES} bytes. No directives (%%{{init: …}}%%), no front matter, \
                 no click, link, links or callback statements, no HTML."
            );
        }
        ReservedKind::Check => {
            let item = &CHECK_SCHEMA["properties"]["questions"]["items"];
            let schema = serde_json::to_string_pretty(item).unwrap_or_else(|_| item.to_string());
            let _ = write!(
                input,
                "{CHECK_FORMAT}\n\n{schema}\n\nThe ladder's rungs:\n\n"
            );
            for rung in &ladder.rungs {
                let _ = writeln!(input, "- {}: {}", rung.id.0, rung.concept);
            }
        }
        ReservedKind::Record => unreachable!("a RawBlock never holds the section record"),
    }
    input
}

/// The answer without a Markdown code fence around it.
fn strip_fence(answer: &str) -> &str {
    let answer = answer.trim();
    answer
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
        .and_then(|inner| inner.split_once('\n'))
        .map_or(answer, |(_, body)| body.trim())
}
