//! The section stream splitter fed chunk sequences: the ordered events for a
//! whole section, independence from where the stream is cut, when a block
//! opens, fences that are not blocks, unclosed input, `alt:` lines and the
//! indices of the shown blocks.

use atlas_core::harness::splitter::{ReservedKind, SplitEvent, Splitter};

/// A section with prose, every reserved block, maths and two ordinary code
/// fences, one of them holding a reserved name as content.
const SECTION: &str = concat!(
    "# Heat engines\n",
    "\n",
    "An engine turns heat into work; its efficiency is $\\eta = 1 - T_c/T_h$.\n",
    "\n",
    "```atlas-visual\n",
    "alt: A timeline of engine designs\n",
    "{\"root\":\"t\",\"elements\":{}}\n",
    "```\n",
    "\n",
    "The Carnot cycle has four steps.\n",
    "\n",
    "```atlas-figure\n",
    "alt: Four strokes on a PV diagram\n",
    "<svg viewBox=\"0 0 10 10\"><path d=\"M0 0L10 10\"/></svg>\n",
    "```\n",
    "```mermaid\n",
    "graph LR\n",
    "  A --> B\n",
    "```\n",
    "\n",
    "A code block stays prose:\n",
    "\n",
    "```text\n",
    "```atlas-check\n",
    "```\n",
    "\n",
    "$$\n",
    "W = Q_h - Q_c\n",
    "$$\n",
    "\n",
    "```atlas-check\n",
    "alt: One question on efficiency\n",
    "{\"questions\":[]}\n",
    "```\n",
    "\n",
    "```atlas-record\n",
    "{\"summary\":\"Engines.\",\"concepts\":[{\"name\":\"efficiency\"}]}\n",
    "```\n",
);

const RECORD_BLOCK: &str = concat!(
    "```atlas-record\n",
    "{\"summary\":\"Engines.\",\"concepts\":[{\"name\":\"efficiency\"}]}\n",
    "```\n",
);

/// Multi-byte characters in prose, in an `alt:` line and in fence payloads,
/// so cuts fall inside them.
const UNICODE_SECTION: &str = concat!(
    "Naïve entropy: ΔS ≥ 0 🔥\n",
    "```atlas-figure\n",
    "alt: Wärme fließt → kalt\n",
    "<svg><text>∂Q/T — é</text></svg>\n",
    "```\n",
    "Après le bloc, «prose».\n",
    "~~~~python\n",
    "print(\"ünïcode\")\n",
    "```mermaid\n",
    "~~~~\n",
    "````mermaid\n",
    "graph LR\n",
    "  É --> ✓\n",
    "```\n",
    "````\n",
    "Fin ✓",
);

fn prose(block: u32, text: &str) -> SplitEvent {
    SplitEvent::Prose {
        block,
        text: text.to_owned(),
    }
}

fn opened(block: Option<u32>, kind: ReservedKind) -> SplitEvent {
    SplitEvent::Opened { block, kind }
}

fn closed(
    block: Option<u32>,
    kind: ReservedKind,
    payload: &str,
    description: Option<&str>,
) -> SplitEvent {
    SplitEvent::Closed {
        block,
        kind,
        payload: payload.to_owned(),
        description: description.map(str::to_owned),
    }
}

/// Prose arrives as it comes, so how it is cut into events depends on the
/// chunks; adjacent prose for the same block is joined before comparing.
fn coalesce(events: Vec<SplitEvent>) -> Vec<SplitEvent> {
    let mut out: Vec<SplitEvent> = Vec::new();
    for event in events {
        if let SplitEvent::Prose { block, text } = &event
            && let Some(SplitEvent::Prose {
                block: last_block,
                text: last_text,
            }) = out.last_mut()
            && last_block == block
        {
            last_text.push_str(text);
            continue;
        }
        out.push(event);
    }
    out
}

fn split_chunks(chunks: &[&[u8]]) -> Vec<SplitEvent> {
    let mut splitter = Splitter::new();
    let mut events = Vec::new();
    for chunk in chunks {
        events.extend(splitter.push(chunk));
    }
    events.extend(splitter.finish());
    coalesce(events)
}

fn split_whole(input: &str) -> Vec<SplitEvent> {
    split_chunks(&[input.as_bytes()])
}

/// A small deterministic generator for cut positions.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Splits `input` whole, then cut at every byte, byte by byte and at random
/// positions, and requires the same events each time. The reference must
/// hold `closed_blocks` closed blocks, so an empty answer cannot pass.
fn assert_cut_independent(input: &str, closed_blocks: usize) {
    let bytes = input.as_bytes();
    let reference = split_whole(input);
    assert_eq!(
        reference
            .iter()
            .filter(|event| matches!(event, SplitEvent::Closed { .. }))
            .count(),
        closed_blocks,
        "closed blocks in {reference:?}"
    );

    for cut in 0..=bytes.len() {
        let (head, tail) = bytes.split_at(cut);
        assert_eq!(split_chunks(&[head, tail]), reference, "cut at byte {cut}");
    }

    let single_bytes: Vec<&[u8]> = bytes.chunks(1).collect();
    assert_eq!(split_chunks(&single_bytes), reference, "byte by byte");

    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for round in 0..300 {
        let mut chunks: Vec<&[u8]> = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let len = (rng.next() % 9) as usize;
            let len = len.min(rest.len());
            let (chunk, tail) = rest.split_at(len);
            chunks.push(chunk);
            rest = tail;
        }
        assert_eq!(split_chunks(&chunks), reference, "random round {round}");
    }
}

#[test]
fn a_whole_section_yields_its_blocks_in_order_with_exact_payloads() {
    use ReservedKind::*;

    assert_eq!(
        split_whole(SECTION),
        vec![
            prose(
                0,
                "# Heat engines\n\nAn engine turns heat into work; its efficiency is $\\eta = 1 - T_c/T_h$.\n\n",
            ),
            opened(Some(1), Visual),
            closed(
                Some(1),
                Visual,
                "{\"root\":\"t\",\"elements\":{}}\n",
                Some("A timeline of engine designs"),
            ),
            prose(2, "\nThe Carnot cycle has four steps.\n\n"),
            opened(Some(3), Figure),
            closed(
                Some(3),
                Figure,
                "<svg viewBox=\"0 0 10 10\"><path d=\"M0 0L10 10\"/></svg>\n",
                Some("Four strokes on a PV diagram"),
            ),
            opened(Some(4), Mermaid),
            closed(Some(4), Mermaid, "graph LR\n  A --> B\n", None),
            prose(
                5,
                "\nA code block stays prose:\n\n```text\n```atlas-check\n```\n\n$$\nW = Q_h - Q_c\n$$\n\n",
            ),
            opened(Some(6), Check),
            closed(
                Some(6),
                Check,
                "{\"questions\":[]}\n",
                Some("One question on efficiency"),
            ),
            opened(None, Record),
            closed(
                None,
                Record,
                "{\"summary\":\"Engines.\",\"concepts\":[{\"name\":\"efficiency\"}]}\n",
                None,
            ),
            SplitEvent::Finished { record_seen: true },
        ]
    );
}

#[test]
fn cutting_the_stream_anywhere_yields_the_same_events() {
    assert_cut_independent(SECTION, 5);
    assert_cut_independent(UNICODE_SECTION, 2);
}

#[test]
fn multi_byte_characters_cut_across_chunks_arrive_whole() {
    use ReservedKind::*;

    assert_eq!(
        split_whole(UNICODE_SECTION),
        vec![
            prose(0, "Naïve entropy: ΔS ≥ 0 🔥\n"),
            opened(Some(1), Figure),
            closed(
                Some(1),
                Figure,
                "<svg><text>∂Q/T — é</text></svg>\n",
                Some("Wärme fließt → kalt"),
            ),
            prose(
                2,
                "Après le bloc, «prose».\n~~~~python\nprint(\"ünïcode\")\n```mermaid\n~~~~\n",
            ),
            opened(Some(3), Mermaid),
            closed(Some(3), Mermaid, "graph LR\n  É --> ✓\n```\n", None),
            prose(4, "Fin ✓"),
            SplitEvent::Finished { record_seen: false },
        ]
    );
}

#[test]
fn a_block_opens_when_its_fence_line_completes_and_prose_after_it_is_a_new_block() {
    use ReservedKind::*;
    let mut splitter = Splitter::new();

    assert_eq!(
        coalesce(splitter.push(b"Some prose")),
        vec![prose(0, "Some prose")],
        "prose is not held back"
    );
    assert_eq!(
        coalesce(splitter.push(b"\n```atlas-fig")),
        vec![prose(0, "\n")],
        "a line that may still open a block is held"
    );
    assert_eq!(
        coalesce(splitter.push(b"ure")),
        vec![],
        "the fence line is not complete yet"
    );
    assert_eq!(
        coalesce(splitter.push(b"\n")),
        vec![opened(Some(1), Figure)],
        "opened as soon as the fence line completes"
    );
    assert_eq!(
        coalesce(splitter.push(b"alt: A dot\n<svg>")),
        vec![],
        "the payload waits for the closing fence"
    );
    assert_eq!(
        coalesce(splitter.push(b"</svg>\n```\nMore prose")),
        vec![
            closed(Some(1), Figure, "<svg></svg>\n", Some("A dot")),
            prose(2, "More prose"),
        ]
    );
    assert_eq!(
        coalesce(splitter.push(b"\n```python\n")),
        vec![prose(2, "\n```python\n")],
        "an ordinary fence stays in the prose"
    );
    assert_eq!(
        coalesce(splitter.finish()),
        vec![SplitEvent::Finished { record_seen: false }]
    );
}

#[test]
fn a_reserved_name_inside_a_longer_outer_fence_is_not_a_block() {
    let backticks = "````markdown\n```mermaid\ngraph LR\n```\n````\nAfter\n";
    assert_eq!(
        split_whole(backticks),
        vec![
            prose(0, backticks),
            SplitEvent::Finished { record_seen: false },
        ]
    );

    let tildes = "~~~\n```atlas-visual\n{}\n```\n~~~\n";
    assert_eq!(
        split_whole(tildes),
        vec![
            prose(0, tildes),
            SplitEvent::Finished { record_seen: false },
        ]
    );
}

#[test]
fn a_longer_reserved_fence_closes_only_on_a_fence_as_long() {
    use ReservedKind::*;

    assert_eq!(
        split_whole("````mermaid\ngraph\n```\nstill\n````\n"),
        vec![
            opened(Some(0), Mermaid),
            closed(Some(0), Mermaid, "graph\n```\nstill\n", None),
            SplitEvent::Finished { record_seen: false },
        ]
    );
    assert_eq!(
        split_whole("~~~atlas-check\n[]\n```\n~~~\n"),
        vec![
            opened(Some(0), Check),
            closed(Some(0), Check, "[]\n```\n", None),
            SplitEvent::Finished { record_seen: false },
        ]
    );
}

#[test]
fn input_ending_inside_a_block_is_unclosed_with_what_arrived() {
    use ReservedKind::*;

    assert_eq!(
        split_whole("Text\n```atlas-visual\nalt: Partial\n{\"root\":"),
        vec![
            prose(0, "Text\n"),
            opened(Some(1), Visual),
            SplitEvent::Unclosed {
                block: Some(1),
                kind: Visual,
                payload: "{\"root\":".to_owned(),
                description: Some("Partial".to_owned()),
            },
            SplitEvent::Finished { record_seen: false },
        ]
    );

    assert_eq!(
        split_whole("```atlas-record\n{\"summary\":"),
        vec![
            opened(None, Record),
            SplitEvent::Unclosed {
                block: None,
                kind: Record,
                payload: "{\"summary\":".to_owned(),
                description: None,
            },
            SplitEvent::Finished { record_seen: true },
        ]
    );
}

#[test]
fn a_closing_fence_at_the_very_end_closes_the_block() {
    use ReservedKind::*;

    assert_eq!(
        split_whole("```mermaid\ngraph\n```"),
        vec![
            opened(Some(0), Mermaid),
            closed(Some(0), Mermaid, "graph\n", None),
            SplitEvent::Finished { record_seen: false },
        ]
    );
}

#[test]
fn input_without_a_record_block_says_so() {
    let without_record = SECTION.replace(RECORD_BLOCK, "");
    assert_ne!(without_record, SECTION);

    let events = split_whole(&without_record);
    assert_eq!(
        events.last(),
        Some(&SplitEvent::Finished { record_seen: false })
    );
    assert!(!events.iter().any(|event| matches!(
        event,
        SplitEvent::Opened {
            kind: ReservedKind::Record,
            ..
        }
    )));
}

#[test]
fn alt_lines_are_extracted_and_absent_from_the_payload() {
    use ReservedKind::*;

    assert_eq!(
        split_whole("```mermaid\nalt: Two boxes joined\ngraph LR\n  A --> B\n```\n"),
        vec![
            opened(Some(0), Mermaid),
            closed(
                Some(0),
                Mermaid,
                "graph LR\n  A --> B\n",
                Some("Two boxes joined"),
            ),
            SplitEvent::Finished { record_seen: false },
        ]
    );

    assert_eq!(
        split_whole("```mermaid\ngraph LR\nalt: not first\n```\n"),
        vec![
            opened(Some(0), Mermaid),
            closed(Some(0), Mermaid, "graph LR\nalt: not first\n", None),
            SplitEvent::Finished { record_seen: false },
        ],
        "only the first line is a description"
    );

    assert_eq!(
        split_whole("```atlas-record\nalt: kept\n{}\n```\n"),
        vec![
            opened(None, Record),
            closed(None, Record, "alt: kept\n{}\n", None),
            SplitEvent::Finished { record_seen: true },
        ],
        "the record takes no description"
    );
}

/// The shown blocks as (index, kind), one entry per block.
fn shown_blocks(events: &[SplitEvent]) -> Vec<(u32, Option<ReservedKind>)> {
    let mut blocks: Vec<(u32, Option<ReservedKind>)> = Vec::new();
    for event in events {
        let entry = match event {
            SplitEvent::Prose { block, .. } => (*block, None),
            SplitEvent::Opened {
                block: Some(block),
                kind,
            } => (*block, Some(*kind)),
            _ => continue,
        };
        if blocks.last() != Some(&entry) {
            blocks.push(entry);
        }
    }
    blocks
}

#[test]
fn shown_block_indices_do_not_depend_on_a_record_block() {
    use ReservedKind::*;

    let without_record = SECTION.replace(RECORD_BLOCK, "");
    assert_eq!(
        shown_blocks(&split_whole(SECTION)),
        shown_blocks(&split_whole(&without_record))
    );

    let with_record = "A\n```atlas-record\n{}\n```\n```mermaid\nx\n```\nB\n";
    let without_record = "A\n```mermaid\nx\n```\nB\n";
    let expected = vec![(0, None), (1, Some(Mermaid)), (2, None)];
    assert_eq!(shown_blocks(&split_whole(with_record)), expected);
    assert_eq!(shown_blocks(&split_whole(without_record)), expected);
}

#[test]
fn bytes_that_are_not_utf8_become_replacement_characters_wherever_cut() {
    let input: &[u8] = b"a\xFFb\n```mermaid\nx\xE2\x82\n```\nc\xF0\x9F";
    let expected = vec![
        prose(0, "a\u{FFFD}b\n"),
        opened(Some(1), ReservedKind::Mermaid),
        closed(Some(1), ReservedKind::Mermaid, "x\u{FFFD}\n", None),
        prose(2, "c\u{FFFD}"),
        SplitEvent::Finished { record_seen: false },
    ];
    for cut in 0..=input.len() {
        let (head, tail) = input.split_at(cut);
        assert_eq!(split_chunks(&[head, tail]), expected, "cut at byte {cut}");
    }
}
