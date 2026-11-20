//! Places the provider's citations, which index the whole output text, on the
//! prose blocks they fall in, and numbers the sources.
//!
//! The caller lays the output out as it reads it: each run of output
//! characters that is the next text of a prose block is reported with
//! [`Citations::prose`]; every other character (a fenced block, whitespace
//! dropped between blocks) belongs to no prose block. Positions are counted
//! in characters, in the output as in a block's text.
//!
//! A citation is placed as soon as the prose laid out reaches past its range,
//! and every citation left is placed by [`Citations::finish`]. Its range is
//! moved onto the block its start falls in and cut at that block's end. A
//! range that starts outside every prose block marks the end of the nearest
//! prose before it, or the start of the first prose after it when none comes
//! before. Sources are numbered from 1 in the order they are first cited; the
//! same URL keeps its number.

use std::collections::HashMap;

use crate::protocol::{Citation, Source};
use crate::provider::ProviderCitation;

/// A version's citations per block and its numbered sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacedCitations {
    pub citations: Vec<Citation>,
    pub sources: Vec<Source>,
}

/// A run of the output that is text of a prose block.
#[derive(Debug, Clone, Copy)]
struct Span {
    /// Where the run starts and ends in the output.
    start: u32,
    end: u32,
    block: u32,
    /// Where the run starts in the block's text.
    at: u32,
}

impl Span {
    /// The position in the block's text of the output position `position`,
    /// kept within the run.
    fn in_block(&self, position: u32) -> u32 {
        self.at + position.clamp(self.start, self.end) - self.start
    }
}

/// The citations of one output as it is read.
#[derive(Debug, Default)]
pub struct Citations {
    /// The prose runs, in output order.
    spans: Vec<Span>,
    /// The length of each prose block's text so far.
    lengths: HashMap<u32, u32>,
    /// Citations whose range the prose laid out does not reach past yet.
    pending: Vec<ProviderCitation>,
    numbers: HashMap<String, u32>,
    placed: PlacedCitations,
}

impl Citations {
    pub fn new() -> Self {
        Self::default()
    }

    /// The `chars` output characters from `start` are the next text of the
    /// prose block `block`.
    pub fn prose(&mut self, start: u32, block: u32, chars: u32) {
        if chars == 0 {
            return;
        }
        let length = self.lengths.entry(block).or_default();
        self.spans.push(Span {
            start,
            end: start + chars,
            block,
            at: *length,
        });
        *length += chars;
    }

    /// Takes a citation, to place once the prose laid out reaches past it.
    pub fn cite(&mut self, citation: ProviderCitation) {
        self.pending.push(citation);
    }

    /// Places the citations the prose laid out so far reaches past, in the
    /// order they came, and answers each with its source.
    pub fn ready(&mut self) -> Vec<(Citation, Source)> {
        let reached = self.spans.last().map_or(0, |span| span.end);
        let (ready, pending) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|citation| citation.end.max(citation.start + 1) <= reached);
        self.pending = pending;
        self.place_all(ready)
    }

    /// The output is complete: places every citation left, and answers each
    /// with its source. A citation of an output without prose is dropped.
    pub fn finish(&mut self) -> Vec<(Citation, Source)> {
        let left = std::mem::take(&mut self.pending);
        self.place_all(left)
    }

    /// Every citation placed, with the sources numbered.
    pub fn into_placed(self) -> PlacedCitations {
        self.placed
    }

    fn place_all(&mut self, citations: Vec<ProviderCitation>) -> Vec<(Citation, Source)> {
        citations
            .into_iter()
            .filter_map(|citation| self.place(citation))
            .collect()
    }

    fn place(&mut self, cited: ProviderCitation) -> Option<(Citation, Source)> {
        let (block, start, end) = self.range(cited.start, cited.end)?;
        let source = self.source(cited.url, cited.title);
        let citation = Citation {
            block,
            start,
            end,
            source: source.number,
        };
        self.placed.citations.push(citation.clone());
        Some((citation, source))
    }

    /// The block and the range in its text of the output range `start..end`.
    fn range(&self, start: u32, end: u32) -> Option<(u32, u32, u32)> {
        let Some(first) = self
            .spans
            .iter()
            .position(|span| span.start <= start && start < span.end)
        else {
            return self.beside(start);
        };
        let block = self.spans[first].block;
        let from = self.spans[first].in_block(start);
        let to = self.spans[first..]
            .iter()
            .take_while(|span| span.block == block && span.start < end)
            .last()
            .map_or(from, |span| span.in_block(end))
            .max(from);
        Some((block, from, to))
    }

    /// The empty range an output position outside every prose block is
    /// placed at: the end of the prose before it, else the start of the prose
    /// after it.
    fn beside(&self, position: u32) -> Option<(u32, u32, u32)> {
        let before = self.spans.iter().rev().find(|span| span.end <= position);
        let span = before.or_else(|| self.spans.iter().find(|span| span.start > position))?;
        let at = if before.is_some() {
            span.in_block(span.end)
        } else {
            span.at
        };
        Some((span.block, at, at))
    }

    /// The numbered source for `url`, numbered now when first cited.
    fn source(&mut self, url: String, title: String) -> Source {
        if let Some(&number) = self.numbers.get(&url) {
            return self.placed.sources[number as usize - 1].clone();
        }
        let source = Source {
            number: self.placed.sources.len() as u32 + 1,
            url,
            title,
        };
        self.numbers.insert(source.url.clone(), source.number);
        self.placed.sources.push(source.clone());
        source
    }
}
