//! The Responses event stream: server-sent events parsed incrementally and
//! mapped to what Atlas uses of them.
//!
//! A response counts only after `response.completed`. `response.failed` (or an
//! `error` event), `response.incomplete` and a stream that ends without any of
//! them are separate outcomes. Each stream ends with exactly one outcome;
//! anything after it is dropped. Event types Atlas does not use are ignored.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::provider::{ProviderCitation, ProviderError, ProviderEvent};

/// What a Responses stream yields, in order. Exactly one outcome
/// (`Completed`, `Failed`, `Incomplete` or `Broken`) ends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponsesEvent {
    TextDelta(String),
    /// A web search call started.
    Searching,
    /// A URL citation; its range counts characters of the concatenated output
    /// text.
    Citation(ProviderCitation),
    Completed,
    Failed(ResponseFailure),
    Incomplete {
        reason: Option<String>,
    },
    /// The stream ended without a terminal event.
    Broken,
}

impl ResponsesEvent {
    /// The event as a provider stream item when nothing retries the request:
    /// a failure is the error state its code maps to, a broken stream is
    /// incomplete, the rest are provider events.
    pub fn into_provider(self) -> Result<ProviderEvent, ProviderError> {
        match self {
            Self::TextDelta(text) => Ok(ProviderEvent::TextDelta(text)),
            Self::Searching => Ok(ProviderEvent::Searching),
            Self::Citation(citation) => Ok(ProviderEvent::Citation(citation)),
            Self::Completed => Ok(ProviderEvent::Completed),
            Self::Incomplete { reason } => Ok(ProviderEvent::Incomplete { reason }),
            Self::Failed(failure) => Err(super::errors::response_failure(failure).settled()),
            Self::Broken => Err(ProviderError::Incomplete { request_id: None }),
        }
    }
}

/// The error a failed response reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseFailure {
    pub code: Option<String>,
    pub message: Option<String>,
    pub param: Option<String>,
    /// The request's id, from the response headers, for diagnostics.
    pub request_id: Option<String>,
}

/// Parses a Responses stream fed in chunks of any size.
#[derive(Debug, Default)]
pub struct ResponsesStreamParser {
    sse: SseDecoder,
    request_id: Option<String>,
    ended: bool,
    /// Characters of output text delivered so far.
    text_chars: u32,
    /// Where each output text part, by output and content index, starts in
    /// the concatenated output text.
    parts: HashMap<(u64, u64), u32>,
    /// The web search calls already reported.
    searches: HashSet<String>,
}

impl ResponsesStreamParser {
    /// A parser for the stream of the request with this id.
    pub fn new(request_id: Option<String>) -> Self {
        Self {
            request_id,
            ..Self::default()
        }
    }

    /// The events completed by these bytes.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<ResponsesEvent> {
        let mut events = Vec::new();
        for message in self.sse.feed(bytes) {
            events.extend(self.map(&message));
        }
        events
    }

    /// The events left when the stream ends, ending with its outcome: a
    /// stream without a terminal event is broken.
    pub fn finish(mut self) -> Vec<ResponsesEvent> {
        let mut events = Vec::new();
        if let Some(message) = self.sse.finish() {
            events.extend(self.map(&message));
        }
        if !self.ended {
            events.push(ResponsesEvent::Broken);
        }
        events
    }

    fn map(&mut self, message: &SseMessage) -> Option<ResponsesEvent> {
        if self.ended {
            return None;
        }
        let data: Value = serde_json::from_str(&message.data).ok()?;
        let kind = data
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or(&message.event);
        let event = match kind {
            "response.output_text.delta" => self.text_delta(&data)?,
            "response.output_text.annotation.added" => self.citation(&data)?,
            "response.web_search_call.in_progress" | "response.web_search_call.searching" => {
                let call = text(&data, "item_id").unwrap_or_default();
                if !self.searches.insert(call) {
                    return None;
                }
                ResponsesEvent::Searching
            }
            "response.completed" => ResponsesEvent::Completed,
            "response.failed" => self.failure(data.pointer("/response/error")),
            "error" => self.failure(Some(data.get("error").unwrap_or(&data))),
            "response.incomplete" => ResponsesEvent::Incomplete {
                reason: data
                    .pointer("/response/incomplete_details/reason")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            },
            _ => return None,
        };
        self.ended = matches!(
            event,
            ResponsesEvent::Completed
                | ResponsesEvent::Failed(_)
                | ResponsesEvent::Incomplete { .. }
        );
        Some(event)
    }

    fn text_delta(&mut self, data: &Value) -> Option<ResponsesEvent> {
        let delta = text(data, "delta").filter(|delta| !delta.is_empty())?;
        let start = self.text_chars;
        self.parts.entry(part(data)).or_insert(start);
        let chars = u32::try_from(delta.chars().count()).ok()?;
        self.text_chars = start.checked_add(chars)?;
        Some(ResponsesEvent::TextDelta(delta))
    }

    fn citation(&self, data: &Value) -> Option<ResponsesEvent> {
        let annotation = data.get("annotation")?;
        if annotation.get("type").and_then(Value::as_str) != Some("url_citation") {
            return None;
        }
        let base = self
            .parts
            .get(&part(data))
            .copied()
            .unwrap_or(self.text_chars);
        let index = |name: &str| {
            let index = u32::try_from(annotation.get(name)?.as_u64()?).ok()?;
            base.checked_add(index)
        };
        Some(ResponsesEvent::Citation(ProviderCitation {
            url: text(annotation, "url")?,
            title: text(annotation, "title").unwrap_or_default(),
            start: index("start_index")?,
            end: index("end_index")?,
        }))
    }

    fn failure(&self, error: Option<&Value>) -> ResponsesEvent {
        let field = |name: &str| error.and_then(|error| text(error, name));
        ResponsesEvent::Failed(ResponseFailure {
            code: field("code"),
            message: field("message"),
            param: field("param"),
            request_id: self.request_id.clone(),
        })
    }
}

/// The output and content index of the text part an event belongs to.
fn part(data: &Value) -> (u64, u64) {
    let index = |name: &str| data.get(name).and_then(Value::as_u64).unwrap_or(0);
    (index("output_index"), index("content_index"))
}

fn text(value: &Value, name: &str) -> Option<String> {
    value.get(name)?.as_str().map(str::to_owned)
}

/// One server-sent event: its type and its data lines joined.
#[derive(Debug, PartialEq, Eq)]
struct SseMessage {
    event: String,
    data: String,
}

/// An incremental server-sent-events decoder: lines end with LF, CRLF or CR,
/// in any chunking; a line starting with `:` is a comment; data lines join
/// with LF; a blank line dispatches the event.
#[derive(Debug, Default)]
struct SseDecoder {
    /// The bytes of the line not yet ended.
    line: Vec<u8>,
    /// The last line ended with CR, so a following LF belongs to it.
    after_cr: bool,
    event: String,
    data: Option<String>,
}

impl SseDecoder {
    fn feed(&mut self, mut bytes: &[u8]) -> Vec<SseMessage> {
        let mut messages = Vec::new();
        if self.after_cr && !bytes.is_empty() {
            self.after_cr = false;
            if bytes[0] == b'\n' {
                bytes = &bytes[1..];
            }
        }
        while let Some(end) = bytes.iter().position(|&b| b == b'\n' || b == b'\r') {
            self.line.extend_from_slice(&bytes[..end]);
            let line = std::mem::take(&mut self.line);
            messages.extend(self.line_ended(&line));
            let mut next = end + 1;
            if bytes[end] == b'\r' {
                match bytes.get(next) {
                    Some(b'\n') => next += 1,
                    Some(_) => {}
                    None => self.after_cr = true,
                }
            }
            bytes = &bytes[next..];
        }
        self.line.extend_from_slice(bytes);
        messages
    }

    /// The event still pending at the end of the stream, ended or not.
    fn finish(&mut self) -> Option<SseMessage> {
        let line = std::mem::take(&mut self.line);
        if !line.is_empty() {
            self.line_ended(&line);
        }
        self.dispatch()
    }

    fn line_ended(&mut self, line: &[u8]) -> Option<SseMessage> {
        if line.is_empty() {
            return self.dispatch();
        }
        if line[0] == b':' {
            return None;
        }
        let line = String::from_utf8_lossy(line);
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_ref(), ""),
        };
        match field {
            "event" => value.clone_into(&mut self.event),
            "data" => match &mut self.data {
                Some(data) => {
                    data.push('\n');
                    data.push_str(value);
                }
                None => self.data = Some(value.to_owned()),
            },
            _ => {}
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseMessage> {
        let event = std::mem::take(&mut self.event);
        Some(SseMessage {
            event,
            data: self.data.take()?,
        })
    }
}
