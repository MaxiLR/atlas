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
//! block. Block quotes and list items are followed as CommonMark follows them:
//! a fence inside one, whatever its name, is an ordinary code fence in the
//! prose, and it ends with the container it sits in. A reserved block opens
//! only outside every container. HTML blocks are not followed: a fence line
//! inside one is read as a fence.
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
    /// The Markdown structure the prose is in.
    outline: Outline,
    record_seen: bool,
}

#[derive(Debug, Default)]
enum Mode {
    /// Markdown prose, ordinary code fences included.
    #[default]
    Prose,
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
                    if !self.line_is_prose
                        && (self.outline.in_top_level_code() || !may_open_reserved(&self.line))
                    {
                        self.line_is_prose = true;
                    }
                    if self.line_is_prose {
                        self.report_line(out);
                    }
                }
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
                match self.outline.line(content) {
                    Some((kind, fence)) => self.open_block(kind, fence, out),
                    None => self.emit_prose(&line[self.reported..], out),
                }
                self.reported = 0;
                self.line_is_prose = false;
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

/// The reserved kind an info string names.
fn reserved_name(info: &str) -> Option<ReservedKind> {
    info.split_ascii_whitespace()
        .next()
        .and_then(ReservedKind::from_name)
}

/// The Markdown structure around the prose, as far as fences need it: the
/// open block quotes and list items, and the leaf block in the innermost one.
/// Lines are read with CommonMark's parsing strategy.
#[derive(Debug, Default)]
struct Outline {
    containers: Vec<Container>,
    leaf: Leaf,
}

#[derive(Debug, Clone, Copy)]
enum Container {
    Quote,
    /// A list item whose content starts `indent` columns in; `empty` until a
    /// line puts something in it.
    Item {
        indent: usize,
        empty: bool,
    },
}

#[derive(Debug, Default, Clone, Copy)]
enum Leaf {
    #[default]
    None,
    Paragraph,
    IndentedCode,
    Fenced(Fence),
}

impl Outline {
    /// Whether the prose is inside a code fence outside every container,
    /// where no line can open a reserved block.
    fn in_top_level_code(&self) -> bool {
        self.containers.is_empty() && matches!(self.leaf, Leaf::Fenced(_))
    }

    /// Reads one complete line, without its line ending, and returns the
    /// reserved block it opens.
    fn line(&mut self, line: &str) -> Option<(ReservedKind, Fence)> {
        let line = expand_tabs(line);
        let mut rest: &str = &line;
        let blank = is_blank(rest);

        let mut matched = 0;
        for container in &self.containers {
            let after = match *container {
                Container::Quote => quote_marker(rest),
                Container::Item { empty, .. } if blank => (!empty).then_some(rest),
                Container::Item { indent, .. } => {
                    (indentation(rest) >= indent).then(|| &rest[indent..])
                }
            };
            let Some(after) = after else { break };
            rest = after;
            matched += 1;
        }
        if !blank {
            for container in &mut self.containers[..matched] {
                if let Container::Item { empty, .. } = container {
                    *empty = false;
                }
            }
        }
        let all_matched = matched == self.containers.len();
        if all_matched {
            match self.leaf {
                Leaf::Fenced(fence) => {
                    if is_closing(rest, &fence) {
                        self.leaf = Leaf::None;
                    }
                    return None;
                }
                Leaf::IndentedCode if blank || indentation(rest) >= 4 => return None,
                _ => {}
            }
        }

        // Whether the line would otherwise continue an open paragraph, which
        // some blocks may not interrupt.
        let in_paragraph = all_matched && !blank && matches!(self.leaf, Leaf::Paragraph);
        let mut started = false;
        let leaf = loop {
            if indentation(rest) >= 4 {
                let after_paragraph = !started && matches!(self.leaf, Leaf::Paragraph);
                break (!is_blank(rest) && !after_paragraph).then_some(Leaf::IndentedCode);
            }
            if let Some(after) = quote_marker(rest) {
                self.close_unmatched(matched, &mut started);
                self.containers.push(Container::Quote);
                rest = after;
                continue;
            }
            if let Some((fence, info)) = fence_opening(rest) {
                self.close_unmatched(matched, &mut started);
                if self.containers.is_empty()
                    && let Some(kind) = reserved_name(info)
                {
                    return Some((kind, fence));
                }
                break Some(Leaf::Fenced(fence));
            }
            let interrupts = in_paragraph && !started;
            if atx_heading(rest) || thematic_break(rest) || (interrupts && setext_underline(rest)) {
                break Some(Leaf::None);
            }
            if let Some((indent, empty)) = list_marker(rest, interrupts) {
                self.close_unmatched(matched, &mut started);
                self.containers.push(Container::Item { indent, empty });
                rest = &rest[indent.min(rest.len())..];
                continue;
            }
            break None;
        };

        match leaf {
            Some(leaf) => {
                self.close_unmatched(matched, &mut started);
                self.leaf = leaf;
            }
            // A lazy continuation line keeps every container open.
            None if !started && !all_matched && !blank && matches!(self.leaf, Leaf::Paragraph) => {}
            None => {
                self.close_unmatched(matched, &mut started);
                self.leaf = if is_blank(rest) {
                    Leaf::None
                } else {
                    Leaf::Paragraph
                };
            }
        }
        None
    }

    /// Closes, on the line's first block start, the containers the line did
    /// not continue, and the leaf block in the innermost one.
    fn close_unmatched(&mut self, matched: usize, started: &mut bool) {
        if !*started {
            *started = true;
            self.containers.truncate(matched);
            self.leaf = Leaf::None;
        }
    }
}

/// The line with tabs expanded to spaces at stops of four columns.
fn expand_tabs(line: &str) -> std::borrow::Cow<'_, str> {
    if !line.contains('\t') {
        return line.into();
    }
    let mut expanded = String::with_capacity(line.len() + 8);
    let mut column = 0;
    for c in line.chars() {
        if c == '\t' {
            let width = 4 - column % 4;
            expanded.extend(std::iter::repeat_n(' ', width));
            column += width;
        } else {
            expanded.push(c);
            column += 1;
        }
    }
    expanded.into()
}

fn is_blank(text: &str) -> bool {
    text.trim_matches([' ', '\t']).is_empty()
}

/// The number of spaces `text` starts with.
fn indentation(text: &str) -> usize {
    marker_run(text, ' ')
}

/// The rest of a line after a block quote marker.
fn quote_marker(text: &str) -> Option<&str> {
    let (_, rest) = fence_indent(text)?;
    let after = rest.strip_prefix('>')?;
    Some(after.strip_prefix(' ').unwrap_or(after))
}

fn atx_heading(text: &str) -> bool {
    fence_indent(text).is_some_and(|(_, rest)| {
        let len = marker_run(rest, '#');
        (1..=6).contains(&len) && (rest.len() == len || rest[len..].starts_with([' ', '\t']))
    })
}

fn thematic_break(text: &str) -> bool {
    fence_indent(text).is_some_and(|(_, rest)| {
        rest.chars()
            .next()
            .filter(|c| matches!(c, '*' | '-' | '_'))
            .is_some_and(|marker| {
                rest.chars().all(|c| matches!(c, ' ' | '\t') || c == marker)
                    && rest.chars().filter(|&c| c == marker).count() >= 3
            })
    })
}

fn setext_underline(text: &str) -> bool {
    fence_indent(text).is_some_and(|(_, rest)| {
        let rest = rest.trim_end_matches([' ', '\t']);
        rest.chars()
            .next()
            .filter(|c| matches!(c, '=' | '-'))
            .is_some_and(|marker| marker_run(rest, marker) == rest.len())
    })
}

/// A list item marker: the column its content starts at and whether the item
/// starts empty. A paragraph may be interrupted only by a non-empty item that
/// is a bullet or numbered 1.
fn list_marker(text: &str, interrupts_paragraph: bool) -> Option<(usize, bool)> {
    let (indent, rest) = fence_indent(text)?;
    let marker_len = match rest.as_bytes().first()? {
        b'-' | b'+' | b'*' => 1,
        b'0'..=b'9' => {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let delimited = matches!(rest.as_bytes().get(digits), Some(b'.' | b')'));
            let starts_at_one = rest[..digits].parse::<u32>() == Ok(1);
            if digits > 9 || !delimited || (interrupts_paragraph && !starts_at_one) {
                return None;
            }
            digits + 1
        }
        _ => return None,
    };
    let after = &rest[marker_len..];
    if !(after.is_empty() || after.starts_with(' ')) {
        return None;
    }
    let empty = is_blank(after);
    if interrupts_paragraph && empty {
        return None;
    }
    let spaces = indentation(after);
    let padding = if empty || spaces > 4 { 1 } else { spaces };
    Some((indent + marker_len + padding, empty))
}
