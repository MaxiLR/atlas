//! Validation and repair of the section record a section closes with
//! ([`crate::domain::record`]), and the conventions it fixes.
//!
//! The record is the model's own account of its section and later prompts
//! carry it, so it is checked like any other untrusted output before it is
//! stored. Only its shape and its conventions are checked: it is never
//! compared with the prose or with the section's map entry.
//!
//! - [`validate`]: JSON of the record's shape within the limits below. A
//!   promise to a section that is not after this one is dropped.
//! - [`resolve`]: valid, the record, confirmed; invalid or missing, one repair
//!   request ([`prompts::REPAIR_RECORD`]) validated again; still invalid, or
//!   the repair failed, the record the map entry predicts, unconfirmed. The
//!   repair spends plan usage, so it is never repeated, and the caller does not
//!   resolve the record of a stopped generation.
//! - [`apply_conventions`]: the record without the notation and terms that
//!   conflict with a convention already fixed, and the conventions it newly
//!   fixes.

use std::fmt::Write as _;

use serde::Deserialize;

use crate::course_map::{merge_conventions, predicted_record};
use crate::domain::conventions::{Convention, ConventionKind, ConventionOrigin};
use crate::domain::course_map::MapEntry;
use crate::domain::record::{
    FixedEntry, Promise, RecordStatus, SectionRecord, StoredRecord, TaughtConcept,
};
use crate::harness::prompts;
use crate::provider::{ModelProvider, ProviderRequest, Task, complete_text};

/// The longest record accepted, in characters of its JSON.
pub const MAX_RECORD_CHARS: usize = 4_000;
/// The longest summary, in characters. The prompt asks for two to three
/// sentences; sentences are not counted.
pub const MAX_SUMMARY_CHARS: usize = 600;
/// The longest concept name or line, symbol or term and its meaning, example
/// or promise, in characters.
pub const MAX_LINE_CHARS: usize = 300;
/// The most entries in each of the record's lists.
pub const MAX_ENTRIES: usize = 12;

/// Why a record was refused, quoted in the repair request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason}")]
pub struct RecordFault {
    pub reason: String,
}

impl RecordFault {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// What [`apply_conventions`] makes of a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedConventions {
    /// The record without its notation and terms that conflict with a
    /// convention already fixed.
    pub record: SectionRecord,
    /// The conventions the record newly fixes, to store, in the order they
    /// came: notation, terms, then running examples.
    pub fixed: Vec<Convention>,
}

/// The record as the model writes it. Every field is optional here so a
/// missing one is refused with a reason of its own.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "a section record object")]
struct WireRecord {
    summary: Option<String>,
    concepts: Option<Vec<WireConcept>>,
    #[serde(default)]
    notation: Vec<WireEntry>,
    #[serde(default)]
    terms: Vec<WireEntry>,
    #[serde(default)]
    examples: Vec<String>,
    #[serde(default)]
    promises: Vec<WirePromise>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "a concept object")]
struct WireConcept {
    name: Option<String>,
    line: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "an object with a key and a meaning")]
struct WireEntry {
    key: Option<String>,
    meaning: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, expecting = "a promise object")]
struct WirePromise {
    section: Option<String>,
    what: Option<String>,
}

/// Checks a record's payload. `later_sections` are the index keys of the
/// sections after this one; a promise to any other key is dropped.
pub fn validate(
    raw: &str,
    later_sections: &[impl AsRef<str>],
) -> Result<SectionRecord, RecordFault> {
    let wire: WireRecord = serde_json::from_str(raw.trim()).map_err(|error| {
        RecordFault::new(if error.is_syntax() || error.is_eof() {
            format!("the record is not JSON: {error}")
        } else {
            format!("the record does not have the record's shape: {error}")
        })
    })?;

    let summary = text(wire.summary, "the summary", MAX_SUMMARY_CHARS)?;

    let concepts = wire.concepts.unwrap_or_default();
    if concepts.is_empty() {
        return Err(RecordFault::new(
            "the record names no concept; at least one is required",
        ));
    }
    let concepts = list(concepts, "concepts")?
        .map(|(at, concept)| {
            let name = text(
                concept.name,
                &format!("the name of concept {at}"),
                MAX_LINE_CHARS,
            )?;
            let line = text(
                concept.line,
                &format!("the line of concept {at} (\"{name}\")"),
                MAX_LINE_CHARS,
            )?;
            Ok(TaughtConcept {
                name,
                line: Some(line),
            })
        })
        .collect::<Result<_, RecordFault>>()?;

    let notation = fixed_entries(wire.notation, "notation", "symbol")?;
    let terms = fixed_entries(wire.terms, "terms", "term")?;

    let examples = list(wire.examples, "examples")?
        .map(|(at, example)| text(Some(example), &format!("example {at}"), MAX_LINE_CHARS))
        .collect::<Result<_, RecordFault>>()?;

    let promises = wire
        .promises
        .into_iter()
        .filter_map(|promise| {
            let section = promise.section?.trim().to_owned();
            later_sections
                .iter()
                .any(|key| key.as_ref().trim() == section)
                .then_some((section, promise.what))
        })
        .collect::<Vec<_>>();
    let promises = list(promises, "promises")?
        .map(|(_, (section, what))| {
            let what = text(
                what,
                &format!("what was promised to section {section}"),
                MAX_LINE_CHARS,
            )?;
            Ok(Promise { section, what })
        })
        .collect::<Result<_, RecordFault>>()?;

    let record = SectionRecord {
        summary: Some(summary),
        concepts,
        notation,
        terms,
        examples,
        promises,
    };
    let length = serde_json::to_string(&record)
        .expect("a section record serialises")
        .chars()
        .count();
    if length > MAX_RECORD_CHARS {
        return Err(RecordFault::new(format!(
            "the record is {length} characters long as JSON; at most {MAX_RECORD_CHARS} are \
             allowed"
        )));
    }
    Ok(record)
}

/// The record to store for a completed section: the model's record
/// (`raw`, absent when the section closed without one), confirmed when valid;
/// otherwise one repair request with the section's own text, the payload,
/// the reason, the format and the map entry, whose answer is validated again;
/// still invalid, or the request failed, the record `entry` predicts,
/// unconfirmed.
pub async fn resolve(
    raw: Option<&str>,
    section_text: &str,
    entry: &MapEntry,
    later_sections: &[impl AsRef<str>],
    provider: &dyn ModelProvider,
    model: Option<&str>,
) -> StoredRecord {
    let fault = match raw.map(|raw| validate(raw, later_sections)) {
        Some(Ok(record)) => return confirmed(record),
        Some(Err(fault)) => fault,
        None => RecordFault::new("the section ended without a record"),
    };
    let request = ProviderRequest {
        model: model.map(str::to_owned),
        ..ProviderRequest::new(
            Task::RepairRecord,
            prompts::REPAIR_RECORD.text(),
            repair_input(raw, &fault, section_text, entry, later_sections),
        )
    };
    let Ok(answer) = complete_text(provider, request, |_| {}).await else {
        return predicted_record(entry);
    };
    match validate(strip_fence(&answer), later_sections) {
        Ok(record) => confirmed(record),
        Err(_) => predicted_record(entry),
    }
}

/// Merges the record's notation, terms and running examples into the course's
/// conventions ([`merge_conventions`]): the first fixing holds. Notation and
/// term entries that conflict with a convention already fixed, or with an
/// earlier entry of the record, are removed from the record; the record's
/// examples are kept as written. The newly fixed conventions take `origin`.
pub fn apply_conventions(
    mut record: SectionRecord,
    existing: &[Convention],
    origin: ConventionOrigin,
) -> AppliedConventions {
    let convention = |kind, key: &str, meaning: &str| Convention {
        kind,
        key: key.to_owned(),
        meaning: meaning.to_owned(),
        origin,
    };
    let incoming: Vec<Convention> = record
        .notation
        .iter()
        .map(|entry| convention(ConventionKind::Notation, &entry.key, &entry.meaning))
        .chain(
            record
                .terms
                .iter()
                .map(|entry| convention(ConventionKind::Terminology, &entry.key, &entry.meaning)),
        )
        .chain(
            record
                .examples
                .iter()
                .map(|example| convention(ConventionKind::RunningExample, example, example)),
        )
        .collect();
    let merge = merge_conventions(existing, &incoming);
    let conflicts = |kind: ConventionKind, entry: &FixedEntry| {
        merge.conflicts.iter().any(|conflict| {
            conflict.incoming.kind == kind
                && conflict.incoming.key == entry.key
                && conflict.incoming.meaning == entry.meaning
        })
    };
    record
        .notation
        .retain(|entry| !conflicts(ConventionKind::Notation, entry));
    record
        .terms
        .retain(|entry| !conflicts(ConventionKind::Terminology, entry));
    AppliedConventions {
        record,
        fixed: merge.accepted,
    }
}

fn confirmed(record: SectionRecord) -> StoredRecord {
    StoredRecord {
        record,
        status: RecordStatus::Confirmed,
    }
}

/// A text that is present, not blank and at most `limit` characters long.
fn text(value: Option<String>, what: &str, limit: usize) -> Result<String, RecordFault> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Err(RecordFault::new(format!("{what} is missing or empty")));
    };
    let length = value.chars().count();
    if length > limit {
        return Err(RecordFault::new(format!(
            "{what} is {length} characters long; at most {limit} are allowed"
        )));
    }
    Ok(value)
}

/// The entries of a list of at most [`MAX_ENTRIES`], numbered from 1.
fn list<T>(entries: Vec<T>, name: &str) -> Result<impl Iterator<Item = (usize, T)>, RecordFault> {
    if entries.len() > MAX_ENTRIES {
        return Err(RecordFault::new(format!(
            "the record lists {} {name}; at most {MAX_ENTRIES} are allowed",
            entries.len()
        )));
    }
    Ok((1..).zip(entries))
}

fn fixed_entries(
    entries: Vec<WireEntry>,
    name: &str,
    one: &str,
) -> Result<Vec<FixedEntry>, RecordFault> {
    list(entries, name)?
        .map(|(at, entry)| {
            let key = text(entry.key, &format!("the key of {one} {at}"), MAX_LINE_CHARS)?;
            let meaning = text(
                entry.meaning,
                &format!("the meaning of {one} {at} (\"{key}\")"),
                MAX_LINE_CHARS,
            )?;
            Ok(FixedEntry { key, meaning })
        })
        .collect()
}

/// The repair request's input: the section's own text, the payload, what was
/// wrong, the format, the map entry and the keys a promise may name.
fn repair_input(
    raw: Option<&str>,
    fault: &RecordFault,
    section_text: &str,
    entry: &MapEntry,
    later_sections: &[impl AsRef<str>],
) -> String {
    let mut input = format!(
        "## The section\n\n{}\n\n## Your record\n\n",
        section_text.trim()
    );
    match raw {
        Some(raw) => input.push_str(raw.trim()),
        None => input.push_str("You wrote no record."),
    }
    let _ = write!(
        input,
        "\n\n## What was wrong\n\n{}\n\n## The format\n\n{}\n\n## The section's plan\n\n",
        fault.reason,
        format_description()
    );
    let _ = writeln!(input, "Introduces: {}", concept_list(&entry.introduces));
    let _ = writeln!(input, "Relies on: {}", concept_list(&entry.relies_on));
    input.push_str("\n## The later sections\n\n");
    if later_sections.is_empty() {
        input.push_str("None: this is the last section, so promise nothing.");
    } else {
        let keys: Vec<&str> = later_sections.iter().map(AsRef::as_ref).collect();
        let _ = write!(
            input,
            "A promise may name these index keys: {}.",
            keys.join(", ")
        );
    }
    input
}

fn concept_list(concepts: &[String]) -> String {
    if concepts.is_empty() {
        "nothing".to_owned()
    } else {
        concepts.join(", ")
    }
}

fn format_description() -> String {
    format!(
        "One JSON object with these fields:\n\
         - \"summary\": two to three sentences, at most {MAX_SUMMARY_CHARS} characters.\n\
         - \"concepts\": the concepts the section taught, at least one, each \
         {{\"name\": …, \"line\": …}} with one line worded as in the section.\n\
         - \"notation\": the symbols the section fixed, each {{\"key\": …, \"meaning\": …}}.\n\
         - \"terms\": the terms the section fixed, each {{\"key\": …, \"meaning\": …}}.\n\
         - \"examples\": the examples and analogies the section used, each a short text.\n\
         - \"promises\": what the section promised a later section would cover, each \
         {{\"section\": the later section's index key, \"what\": …}}.\n\
         Each list holds at most {MAX_ENTRIES} entries, each name, line, meaning, example and \
         promise at most {MAX_LINE_CHARS} characters, and the whole record at most \
         {MAX_RECORD_CHARS} characters. No other field."
    )
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
