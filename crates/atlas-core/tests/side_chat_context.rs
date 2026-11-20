//! Side-chat context: what a side-chat reply request carries, how older turns
//! are condensed over budget with no request, and that the course memory never
//! reaches it.

mod support;

use atlas_core::harness::chat_context::{
    ChatContext, ChatPartKind, ChatPassage, ChatTurn, OPENING_CHARS, side_chat_context,
};
use atlas_core::harness::context::{Budget, render_profile};
use atlas_core::index_tree::in_index_order;
use atlas_core::protocol::ChatRole;
use support::sample_course::{SampleCourse, sample_course};

const PASSAGE: &str = "The lithosphere is broken into plates that move as rigid pieces.";
const SECTION_TEXT: &str = "# What a plate is\n\nThe lithosphere is broken into plates that \
     move as rigid pieces. SECTION-TEXT-7d1e\n\n```figure\n<svg></svg>\n```\n\nThey move a few \
     centimetres a year, about as fast as fingernails grow.";

fn turn(role: ChatRole, text: &str) -> ChatTurn {
    ChatTurn {
        role,
        text: text.to_owned(),
        stopped: false,
    }
}

fn candidate(text: &str) -> ChatTurn {
    turn(ChatRole::Candidate, text)
}

fn agent(text: &str) -> ChatTurn {
    turn(ChatRole::Agent, text)
}

fn passage() -> Option<ChatPassage<'static>> {
    Some(ChatPassage {
        quote: PASSAGE,
        section_text: SECTION_TEXT,
    })
}

fn build(
    course: &SampleCourse,
    passage: Option<ChatPassage<'_>>,
    history: &[ChatTurn],
    budget: Budget,
) -> ChatContext {
    side_chat_context(
        passage,
        &course.index,
        &course.ladder,
        &course.profile,
        history,
        budget,
    )
}

/// The input's length in characters with nothing condensed.
fn full_size(course: &SampleCourse, history: &[ChatTurn]) -> usize {
    let context = build(course, passage(), history, Budget(usize::MAX));
    assert_eq!(context.condensed, 0);
    context.input().chars().count()
}

/// Every chapter, section and deep-dive title of the course.
fn titles(course: &SampleCourse) -> Vec<String> {
    let mut titles: Vec<String> = course
        .index
        .chapters
        .iter()
        .map(|chapter| chapter.title.clone())
        .collect();
    titles.extend(
        in_index_order(&course.index)
            .iter()
            .map(|entry| entry.section.title.clone()),
    );
    titles
}

/// Sixty turns, each opening with a short sentence naming it and going on
/// with details only it holds.
fn long_history() -> Vec<ChatTurn> {
    (0..60)
        .map(|at| {
            let details: Vec<String> = (0..90).map(|k| format!("Detail {at:02}-{k}")).collect();
            let text = format!("Turn {at:02} opens here. {}", details.join(" "));
            if at % 2 == 0 {
                candidate(&text)
            } else {
                agent(&text)
            }
        })
        .collect()
}

#[test]
fn an_anchored_chat_carries_its_passage_section_titles_profile_and_every_turn() {
    let course = sample_course();
    let mut stopped = agent("Plates move because the mantle below them slowly");
    stopped.stopped = true;
    let history = vec![
        candidate("Why are the plates rigid?"),
        agent("Because the lithosphere is cold and strong. It bends only a little."),
        candidate("And what moves them?"),
        stopped,
        candidate("Could you finish that answer?"),
    ];

    let context = build(&course, passage(), &history, Budget::SIDE_CHAT);
    let input = context.input();

    assert_eq!(context.condensed, 0);
    assert!(context.part(ChatPartKind::Passage).is_some(), "{input}");
    assert!(input.contains(PASSAGE), "{input}");
    assert!(input.contains(SECTION_TEXT), "{input}");
    for title in titles(&course) {
        assert!(input.contains(&title), "missing {title}: {input}");
    }
    assert!(
        input.contains(&render_profile(&course.ladder, &course.profile)),
        "{input}"
    );
    assert!(
        input.contains("## Candidate\nWhy are the plates rigid?"),
        "{input}"
    );
    assert!(
        input.contains(
            "## Atlas\nBecause the lithosphere is cold and strong. It bends only a little."
        ),
        "{input}"
    );
    assert!(
        input.contains(
            "## Atlas, reply stopped before it finished\n\
             Plates move because the mantle below them slowly"
        ),
        "{input}"
    );
    let positions: Vec<usize> = history
        .iter()
        .map(|turn| input.find(&turn.text).expect("every turn is sent"))
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{input}"
    );
}

#[test]
fn a_long_history_fits_the_budget_with_the_oldest_turns_condensed_and_the_newest_whole() {
    let course = sample_course();
    let history = long_history();
    assert!(full_size(&course, &history) > Budget::SIDE_CHAT.0);

    let context = build(&course, passage(), &history, Budget::SIDE_CHAT);
    let input = context.input();

    assert!(input.chars().count() <= Budget::SIDE_CHAT.0);
    assert!(context.condensed > 0, "{input}");
    assert!(context.condensed < history.len() - 1, "{input}");
    for (at, turn) in history.iter().enumerate() {
        if at < context.condensed {
            assert!(
                input.contains(&format!(", condensed\nTurn {at:02} opens here.\n\n## ")),
                "turn {at} is its opening sentence: {input}"
            );
            assert!(!input.contains(&format!("Detail {at:02}-")), "{input}");
        } else {
            assert!(input.contains(&turn.text), "turn {at} is whole: {input}");
        }
    }
    let positions: Vec<usize> = (0..history.len())
        .map(|at| {
            input
                .find(&format!("Turn {at:02} opens here."))
                .expect("every turn is sent")
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{input}"
    );
    assert!(input.contains(PASSAGE));
    assert!(input.contains(SECTION_TEXT));
    assert!(input.contains(&render_profile(&course.ladder, &course.profile)));
}

#[test]
fn condensing_starts_at_the_oldest_turn_and_stops_as_soon_as_the_context_fits() {
    let course = sample_course();
    let history = long_history();
    let full = full_size(&course, &history);
    let whole = build(&course, passage(), &history, Budget(usize::MAX)).input();

    let at_size = build(&course, passage(), &history, Budget(full));
    assert_eq!(at_size.condensed, 0);
    assert_eq!(at_size.input(), whole);

    let one_over = build(&course, passage(), &history, Budget(full - 1));
    assert_eq!(one_over.condensed, 1);
    let input = one_over.input();
    assert!(!input.contains("Detail 00-"), "{input}");
    for turn in &history[1..] {
        assert!(input.contains(&turn.text), "{input}");
    }
}

#[test]
fn the_passage_the_profile_and_the_newest_turn_are_never_shortened() {
    let course = sample_course();
    let history = long_history();

    let context = build(&course, passage(), &history, Budget(0));
    let input = context.input();

    assert_eq!(context.condensed, history.len() - 1);
    assert!(input.contains(PASSAGE), "{input}");
    assert!(input.contains(SECTION_TEXT), "{input}");
    assert!(input.contains(&render_profile(&course.ladder, &course.profile)));
    assert!(input.contains(&history[history.len() - 1].text), "{input}");
    for at in 0..history.len() - 1 {
        assert!(!input.contains(&format!("Detail {at:02}-")), "{input}");
    }
}

#[test]
fn condensing_is_deterministic() {
    let course = sample_course();
    let history = long_history();

    let first = build(&course, passage(), &history, Budget::SIDE_CHAT);
    let second = build(&course, passage(), &history, Budget::SIDE_CHAT);

    assert!(first.condensed > 0, "{first:#?}");
    assert_eq!(first, second);
    assert_eq!(first.input(), second.input());
}

/// The input once only the first of `turns` is condensed.
fn first_condensed(course: &SampleCourse, oldest: &str) -> String {
    let history = vec![
        candidate(oldest),
        agent("A short reply."),
        candidate("A short question?"),
    ];
    let full = full_size(course, &history);
    let context = build(course, passage(), &history, Budget(full - 1));
    assert_eq!(context.condensed, 1);
    context.input()
}

#[test]
fn a_turn_without_a_sentence_end_is_cut_at_the_cap() {
    let course = sample_course();
    let words: Vec<String> = (0..300).map(|at| format!("w{at:03}")).collect();
    let text = words.join(" ");

    let input = first_condensed(&course, &text);

    let kept: String = text.chars().take(OPENING_CHARS).collect();
    let one_more: String = text.chars().take(OPENING_CHARS + 1).collect();
    assert!(
        input.contains(&format!("## Candidate, condensed\n{kept}\n\n")),
        "{input}"
    );
    assert!(!input.contains(&one_more), "{input}");
}

#[test]
fn a_mark_inside_a_word_or_number_does_not_end_the_sentence() {
    let course = sample_course();
    let text = "Plates move about 2.5 cm a year, see e.g.the Atlantic. Its floor \
                widens from the ridge in the middle, which you can follow on any map.";

    let input = first_condensed(&course, text);

    assert!(
        input.contains(
            "## Candidate, condensed\nPlates move about 2.5 cm a year, see e.g.the Atlantic.\n\n"
        ),
        "{input}"
    );
    assert!(!input.contains("Its floor"), "{input}");
}

#[test]
fn a_turn_in_a_script_without_latin_punctuation_is_cut_at_its_own_terminator() {
    let course = sample_course();
    for (text, opening, rest) in [
        (
            "पृथ्वी की तीन परतें हैं। सबसे ऊपर पपड़ी है, जो बहुत पतली है। उसके नीचे मेंटल है।",
            "पृथ्वी की तीन परतें हैं।",
            "सबसे ऊपर",
        ),
        (
            "هل تتحرك الصفائح؟ نعم، تتحرك ببطء شديد، بضعة سنتيمترات كل عام فوق طبقة لينة.",
            "هل تتحرك الصفائح؟",
            "نعم",
        ),
    ] {
        let input = first_condensed(&course, text);

        assert!(
            input.contains(&format!("## Candidate, condensed\n{opening}\n\n")),
            "{input}"
        );
        assert!(!input.contains(rest), "{input}");
    }
}

#[test]
fn a_chat_without_a_selection_has_no_passage_and_no_section_text() {
    let course = sample_course();
    let history = vec![candidate("What is a plate, in one line?")];

    let context = build(&course, None, &history, Budget::SIDE_CHAT);
    let input = context.input();

    assert_eq!(context.part(ChatPartKind::Passage), None);
    assert_eq!(context.part(ChatPartKind::Section), None);
    assert!(!input.contains(PASSAGE), "{input}");
    assert!(!input.contains("SECTION-TEXT-7d1e"), "{input}");
    for title in titles(&course) {
        assert!(input.contains(&title), "missing {title}: {input}");
    }
    assert!(input.contains(&render_profile(&course.ladder, &course.profile)));
    assert!(
        input.contains("## Candidate\nWhat is a plate, in one line?"),
        "{input}"
    );
}

#[test]
fn the_course_memory_never_reaches_a_side_chat() {
    const RECORD_SENTINEL: &str = "SENTINEL-RECORD-5b2a";
    const CONVENTION_SENTINEL: &str = "SENTINEL-CONVENTION-5b2a";
    const CONCEPT_SENTINEL: &str = "SENTINEL-CONCEPT-5b2a";
    const LINE_SENTINEL: &str = "SENTINEL-LINE-5b2a";
    let mut course = sample_course();
    for sample in &mut course.records {
        let record = &mut sample.record.record;
        record.summary = Some(format!("Summary {RECORD_SENTINEL}"));
        for concept in &mut record.concepts {
            concept.line = Some(format!("Line {RECORD_SENTINEL}"));
        }
        record.examples.push(format!("Example {RECORD_SENTINEL}"));
    }
    for convention in &mut course.conventions {
        convention.meaning = format!("{} {CONVENTION_SENTINEL}", convention.meaning);
    }
    for concept in &mut course.map.concepts {
        concept.name = format!("{} {CONCEPT_SENTINEL}", concept.name);
    }
    for entry in &mut course.map.entries {
        entry.introduces.push(CONCEPT_SENTINEL.to_owned());
        entry.relies_on.push(CONCEPT_SENTINEL.to_owned());
    }
    fn mark(sections: &mut [atlas_core::index_tree::TreeSection]) {
        for section in sections {
            section.line = format!("{} {LINE_SENTINEL}", section.line);
            mark(&mut section.children);
        }
    }
    for chapter in &mut course.index.chapters {
        mark(&mut chapter.sections);
    }
    let history = vec![
        candidate("Which conventions does the course use?"),
        agent("I can only see this passage and its section."),
        candidate("And what did the earlier sections teach?"),
    ];

    for passage in [passage(), None] {
        for budget in [Budget::SIDE_CHAT, Budget(0)] {
            let input = build(&course, passage, &history, budget).input();
            assert!(!input.is_empty());
            for sentinel in [
                RECORD_SENTINEL,
                CONVENTION_SENTINEL,
                CONCEPT_SENTINEL,
                LINE_SENTINEL,
            ] {
                assert!(!input.contains(sentinel), "{sentinel}: {input}");
            }
        }
    }
}
