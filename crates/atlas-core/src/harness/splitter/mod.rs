//! Splits a section's stream into blocks as it arrives: Markdown prose, and a
//! reserved fenced block for everything that is not prose.
//!
//! # The section format
//!
//! A section is Markdown prose with a fenced block for everything that is not
//! prose. A block opens with a line of three or more backticks followed by its
//! name, and closes with a line of as many backticks and nothing else. The
//! reserved names are:
//!
//! - `atlas-visual`: a catalog visual, a json-render spec as JSON.
//! - `atlas-figure`: a figure, one static SVG.
//! - `mermaid`: a Mermaid diagram.
//! - `atlas-check`: the check questions.
//! - `atlas-record`: the closing section record as JSON. It is never shown.
//!
//! Every reserved block except the record may start with one line
//! `alt: …`, a short plain-text description of the block. It is kept apart
//! from the payload and stands in for the block when the block cannot be
//! shown.
//!
//! ````markdown
//! Heat flows from hot to cold.
//!
//! ```atlas-figure
//! alt: Two boxes, an arrow from the hot one to the cold one
//! <svg viewBox="0 0 100 40">…</svg>
//! ```
//!
//! ```atlas-record
//! {"summary": "…"}
//! ```
//! ````
//!
//! Any other fence, code in any language or none, is part of the prose, and so
//! is maths written with `$…$` or `$$…$$`.
//!
//! # Splitting
//!
//! Fences follow CommonMark: an opening fence may be indented up to three
//! spaces, may use backticks or tildes, and closes only on a fence of the same
//! character at least as long. Inside an ordinary code fence nothing opens a
//! block. Containers such as block quotes and list items are not tracked, so a
//! reserved fence inside one is not recognised as a block.
//!
//! Prose is reported as it arrives. Only the start of a line that may still
//! become a reserved opening fence is held until the line rules it out or
//! completes. A block is reported opened as soon as its opening fence line is
//! complete, and closed with its raw payload and description when its closing
//! fence arrives; a block still open when the input ends is reported unclosed
//! with what arrived.
//!
//! The shown blocks (each prose run between reserved blocks, and every
//! reserved block but the record) are numbered from 0 in order. The record
//! takes no number and does not end the prose run it sits in, so the numbers
//! are the same with or without it. Whitespace between two blocks starts no
//! prose run of its own: it is held, and dropped unless prose follows it.
//!
//! The splitter reads bytes and is independent of how the stream is cut: a
//! character split across chunks is decoded whole, and bytes that are not
//! UTF-8 become U+FFFD.

/// The reserved fence names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservedKind {
    /// `atlas-visual`: a json-render spec as JSON.
    Visual,
    /// `atlas-figure`: an SVG.
    Figure,
    /// `mermaid`.
    Mermaid,
    /// `atlas-check`: the check questions.
    Check,
    /// `atlas-record`: the closing section record, never shown.
    Record,
}

impl ReservedKind {
    const ALL: [Self; 5] = [
        Self::Visual,
        Self::Figure,
        Self::Mermaid,
        Self::Check,
        Self::Record,
    ];

    /// The fence name that opens a block of this kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Visual => "atlas-visual",
            Self::Figure => "atlas-figure",
            Self::Mermaid => "mermaid",
            Self::Check => "atlas-check",
            Self::Record => "atlas-record",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// What the splitter reports, in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitEvent {
    /// Prose text for the shown block `block`, appended to what came before.
    Prose { block: u32, text: String },
    /// A reserved fence opened. The record takes no shown index.
    Opened {
        block: Option<u32>,
        kind: ReservedKind,
    },
    /// A reserved fence closed with its payload and `alt:` description.
    Closed {
        block: Option<u32>,
        kind: ReservedKind,
        payload: String,
        description: Option<String>,
    },
    /// The input ended inside a reserved fence; the payload is what arrived.
    Unclosed {
        block: Option<u32>,
        kind: ReservedKind,
        payload: String,
        description: Option<String>,
    },
    /// The input ended; `record_seen` tells whether a record block opened.
    Finished { record_seen: bool },
}

/// The incremental splitter: [`Splitter::push`] each chunk as it arrives,
/// then [`Splitter::finish`] once.
#[derive(Debug, Default)]
pub struct Splitter {
    /// The bytes of a character cut off at the end of the last chunk.
    undecoded: Vec<u8>,
    /// The current line, up to and including its newline.
    line: String,
    /// In prose, how much of `line` has been reported.
    reported: usize,
    /// In prose, whether `line` has ruled out opening a reserved block.
    line_is_prose: bool,
    mode: Mode,
    next_index: u32,
    /// The index of the current prose run, once it holds more than whitespace.
    prose_block: Option<u32>,
    /// Whitespace at the start of a prose run not numbered yet.
    held_space: String,
    record_seen: bool,
}

#[derive(Debug, Default)]
enum Mode {
    #[default]
    Prose,
    /// Inside an ordinary code fence, which is prose.
    Code(Fence),
    Block(OpenBlock),
}

#[derive(Debug, Clone, Copy)]
struct Fence {
    marker: char,
    len: usize,
    indent: usize,
}

#[derive(Debug)]
struct OpenBlock {
    block: Option<u32>,
    kind: ReservedKind,
    fence: Fence,
    payload: String,
    description: Option<String>,
    at_first_line: bool,
}

impl OpenBlock {
    /// Adds a content line: the `alt:` description when it is the first line
    /// of a shown block, the payload otherwise.
    fn add_line(&mut self, line: &str) {
        let text = strip_indent(line, self.fence.indent);
        let description = if self.at_first_line && self.kind != ReservedKind::Record {
            text.trim_start().strip_prefix("alt:")
        } else {
            None
        };
        match description.map(str::trim) {
            Some(description) => {
                self.description = (!description.is_empty()).then(|| description.to_owned());
            }
            None => self.payload.push_str(text),
        }
        self.at_first_line = false;
    }
}

/// What stands for bytes that are not UTF-8.
const REPLACEMENT: &str = "\u{FFFD}";

impl Splitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the next chunk and reports what it completes.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SplitEvent> {
        let mut out = Vec::new();
        let mut bytes = std::mem::take(&mut self.undecoded);
        bytes.extend_from_slice(chunk);
        let mut rest = bytes.as_slice();
        loop {
            let (valid, invalid) = match std::str::from_utf8(rest) {
                Ok(text) => (text, None),
                Err(error) => {
                    let (valid, after) = rest.split_at(error.valid_up_to());
                    // Everything before `valid_up_to` is valid UTF-8.
                    let valid = std::str::from_utf8(valid).unwrap_or_default();
                    (valid, Some((error.error_len(), after)))
                }
            };
            self.feed(valid, &mut out);
            match invalid {
                None => break,
                Some((Some(len), after)) => {
                    self.feed(REPLACEMENT, &mut out);
                    rest = &after[len..];
                }
                Some((None, after)) => {
                    self.undecoded = after.to_vec();
                    break;
                }
            }
        }
        out
    }

    /// Ends the input: completes the last line, reports a block still open as
    /// unclosed, and reports whether a record block was seen.
    pub fn finish(mut self) -> Vec<SplitEvent> {
        let mut out = Vec::new();
        if !self.undecoded.is_empty() {
            self.feed(REPLACEMENT, &mut out);
        }
        if !self.line.is_empty() {
            self.end_line(&mut out);
        }
        if let Mode::Block(open) = std::mem::take(&mut self.mode) {
            out.push(SplitEvent::Unclosed {
                block: open.block,
                kind: open.kind,
                payload: open.payload,
                description: open.description,
            });
        }
        out.push(SplitEvent::Finished {
            record_seen: self.record_seen,
        });
        out
    }

    fn feed(&mut self, text: &str, out: &mut Vec<SplitEvent>) {
        for segment in text.split_inclusive('\n') {
            self.line.push_str(segment);
            match self.mode {
                Mode::Prose => {
                    if !self.line_is_prose && !may_open_reserved(&self.line) {
                        self.line_is_prose = true;
                    }
                    if self.line_is_prose {
                        self.report_line(out);
                    }
                }
                Mode::Code(_) => self.emit_prose(segment, out),
                Mode::Block(_) => {}
            }
            if segment.ends_with('\n') {
                self.end_line(out);
            }
        }
    }

    /// Reports the part of the current prose line not reported yet.
    fn report_line(&mut self, out: &mut Vec<SplitEvent>) {
        let line = std::mem::take(&mut self.line);
        self.emit_prose(&line[self.reported..], out);
        self.reported = line.len();
        self.line = line;
    }

    fn end_line(&mut self, out: &mut Vec<SplitEvent>) {
        let mut line = std::mem::take(&mut self.line);
        let content = line.strip_suffix('\n').unwrap_or(&line);
        let content = content.strip_suffix('\r').unwrap_or(content);
        match &mut self.mode {
            Mode::Prose => {
                let reserved = if self.line_is_prose {
                    None
                } else {
                    reserved_opening(content)
                };
                match reserved {
                    Some((kind, fence)) => self.open_block(kind, fence, out),
                    None => {
                        self.emit_prose(&line[self.reported..], out);
                        if let Some((fence, _)) = fence_opening(content) {
                            self.mode = Mode::Code(fence);
                        }
                    }
                }
                self.reported = 0;
                self.line_is_prose = false;
            }
            Mode::Code(fence) => {
                if is_closing(content, fence) {
                    self.mode = Mode::Prose;
                }
            }
            Mode::Block(open) => {
                if !is_closing(content, &open.fence) {
                    open.add_line(&line);
                } else if let Mode::Block(open) = std::mem::take(&mut self.mode) {
                    out.push(SplitEvent::Closed {
                        block: open.block,
                        kind: open.kind,
                        payload: open.payload,
                        description: open.description,
                    });
                }
            }
        }
        line.clear();
        self.line = line;
    }

    fn open_block(&mut self, kind: ReservedKind, fence: Fence, out: &mut Vec<SplitEvent>) {
        let block = if kind == ReservedKind::Record {
            self.record_seen = true;
            None
        } else {
            self.prose_block = None;
            self.held_space.clear();
            Some(self.take_index())
        };
        out.push(SplitEvent::Opened { block, kind });
        self.mode = Mode::Block(OpenBlock {
            block,
            kind,
            fence,
            payload: String::new(),
            description: None,
            at_first_line: true,
        });
    }

    fn take_index(&mut self) -> u32 {
        let index = self.next_index;
        self.next_index += 1;
        index
    }

    fn emit_prose(&mut self, text: &str, out: &mut Vec<SplitEvent>) {
        if text.is_empty() {
            return;
        }
        let block = match self.prose_block {
            Some(block) => block,
            None => {
                self.held_space.push_str(text);
                if self.held_space.chars().all(char::is_whitespace) {
                    return;
                }
                let block = self.take_index();
                self.prose_block = Some(block);
                let text = std::mem::take(&mut self.held_space);
                out.push(SplitEvent::Prose { block, text });
                return;
            }
        };
        if let Some(SplitEvent::Prose {
            block: last,
            text: last_text,
        }) = out.last_mut()
            && *last == block
        {
            last_text.push_str(text);
        } else {
            out.push(SplitEvent::Prose {
                block,
                text: text.to_owned(),
            });
        }
    }
}

/// The indentation of a line that may hold a fence (up to three spaces) and
/// the rest of the line; `None` when it is indented further.
fn fence_indent(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    (indent <= 3 && !rest.starts_with('\t')).then_some((indent, rest))
}

/// The length of the run of `marker` at the start of `text`.
fn marker_run(text: &str, marker: char) -> usize {
    text.len() - text.trim_start_matches(marker).len()
}

/// An opening code fence and its info string, as CommonMark reads it.
fn fence_opening(line: &str) -> Option<(Fence, &str)> {
    let (indent, rest) = fence_indent(line)?;
    let marker = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = marker_run(rest, marker);
    let info = rest[len..].trim_matches([' ', '\t']);
    (len >= 3 && !(marker == '`' && info.contains('`'))).then_some((
        Fence {
            marker,
            len,
            indent,
        },
        info,
    ))
}

/// An opening fence whose info string names a reserved block.
fn reserved_opening(line: &str) -> Option<(ReservedKind, Fence)> {
    let (fence, info) = fence_opening(line)?;
    let name = info.split_ascii_whitespace().next()?;
    ReservedKind::from_name(name).map(|kind| (kind, fence))
}

/// Whether a partial line may still become a reserved opening fence.
fn may_open_reserved(partial: &str) -> bool {
    let Some((_, rest)) = fence_indent(partial) else {
        return false;
    };
    let Some(marker) = rest.chars().next() else {
        return true;
    };
    if !matches!(marker, '`' | '~') {
        return false;
    }
    let len = marker_run(rest, marker);
    if len == rest.len() {
        return true;
    }
    let after = &rest[len..];
    if len < 3 || (marker == '`' && after.contains('`')) {
        return false;
    }
    let info = after.trim_start_matches([' ', '\t']);
    match info.find(|c: char| c.is_ascii_whitespace()) {
        Some(end) => ReservedKind::from_name(&info[..end]).is_some(),
        None => ReservedKind::ALL
            .iter()
            .any(|kind| kind.name().starts_with(info)),
    }
}

/// Whether `line` closes a block opened by `fence`.
fn is_closing(line: &str, fence: &Fence) -> bool {
    fence_indent(line).is_some_and(|(_, rest)| {
        let len = marker_run(rest, fence.marker);
        len >= fence.len && rest[len..].trim_matches([' ', '\t']).is_empty()
    })
}

/// A content line without the opening fence's indentation, as CommonMark
/// removes it.
fn strip_indent(line: &str, indent: usize) -> &str {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    &line[spaces.min(indent)..]
}
