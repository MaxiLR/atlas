//! The Responses event stream: server-sent events parsed incrementally and
//! mapped to what Atlas uses of them.

use crate::provider::ProviderCitation;

/// What a Responses stream yields, in order. Exactly one outcome
/// (`Completed`, `Failed`, `Incomplete` or `Broken`) ends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponsesEvent {
    TextDelta(String),
    /// A web search call made progress.
    Searching,
    /// A URL citation; its range indexes the concatenated output text.
    Citation(ProviderCitation),
    Completed,
    Failed(ResponseFailure),
    Incomplete {
        reason: Option<String>,
    },
    /// The stream ended without a terminal event.
    Broken,
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
pub struct ResponsesStreamParser {}

impl ResponsesStreamParser {
    /// A parser for the stream of the request with this id.
    pub fn new(request_id: Option<String>) -> Self {
        let _ = request_id;
        Self {}
    }

    /// The events completed by these bytes.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<ResponsesEvent> {
        let _ = bytes;
        Vec::new()
    }

    /// The events left when the stream ends, ending with its outcome.
    pub fn finish(self) -> Vec<ResponsesEvent> {
        Vec::new()
    }
}
