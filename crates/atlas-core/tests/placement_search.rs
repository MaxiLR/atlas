//! The placement search of ADR-0006 as pure functions: the first rung for each
//! self-rating, the rung after each answer, when the search stops, the profile
//! it leaves and the check-question step. Rungs are counted from 0 at the most
//! basic; expected rungs are worked out by hand from the spec's rules.

// The exhaustive walk takes its visitor as a plain `dyn FnMut`.
#![allow(clippy::type_complexity)]

use atlas_core::domain::ladder::{Ladder, Rung, RungId};
use atlas_core::domain::profile::{CandidateProfile, Knowledge, Misconception, RungKnowledge};
use atlas_core::placement::{
    SearchAnswer, SearchStep, UnknownRung, apply_check, is_finished, next_rung, profile, step,
};
use atlas_core::protocol::{Goal, SelfRating};

use SelfRating::{Expert, KnowTheBasics, NewToThis, UseItRegularly};

const RATINGS: [SelfRating; 4] = [NewToThis, KnowTheBasics, UseItRegularly, Expert];

fn r() -> SearchAnswer {
    SearchAnswer::Right
}

fn w() -> SearchAnswer {
    SearchAnswer::Wrong {
        misconception: None,
    }
}

fn wm(text: &str) -> SearchAnswer {
    SearchAnswer::Wrong {
        misconception: Some(text.to_owned()),
    }
}

fn s() -> SearchAnswer {
    SearchAnswer::NotSure
}

/// The rung asked for each answer, checking that the search is not finished
/// before the last answer and is finished after it.
fn walk(len: usize, rating: SelfRating, answers: &[SearchAnswer]) -> Vec<usize> {
    let mut asked = Vec::new();
    for k in 0..answers.len() {
        let prefix = &answers[..k];
        assert!(
            !is_finished(len, rating, prefix),
            "finished after {k} answers: {rating:?} on {len} rungs, {answers:?}"
        );
        asked.push(next_rung(len, rating, prefix).expect("a rung to ask"));
    }
    assert!(
        is_finished(len, rating, answers),
        "not finished after {answers:?}: {rating:?} on {len} rungs, asked {asked:?}"
    );
    assert_eq!(next_rung(len, rating, answers), None);
    asked
}

fn ladder(len: usize) -> Ladder {
    Ladder {
        rungs: (0..len)
            .map(|i| Rung {
                id: RungId(i as u32 + 1),
                concept: format!("concept {i}"),
            })
            .collect(),
    }
}

/// Rung `i` (from 0) of `ladder(len)`.
fn id(i: usize) -> RungId {
    RungId(i as u32 + 1)
}

fn states(profile: &CandidateProfile) -> Vec<Knowledge> {
    profile.rungs.iter().map(|rung| rung.state).collect()
}

fn misconception(rung: usize, text: &str) -> Misconception {
    Misconception {
        rung: id(rung),
        text: text.to_owned(),
    }
}

use Knowledge::{Known as K, Shaky as Y, Unknown as U};

#[test]
fn the_first_question_is_at_the_rung_the_self_rating_maps_to() {
    let table: [(usize, [usize; 4]); 3] =
        [(8, [0, 2, 5, 7]), (10, [0, 3, 6, 9]), (12, [0, 4, 7, 11])];
    for (len, firsts) in table {
        for (rating, first) in RATINGS.into_iter().zip(firsts) {
            assert_eq!(
                next_rung(len, rating, &[]),
                Some(first),
                "{rating:?} on {len} rungs"
            );
            assert!(!is_finished(len, rating, &[]));
        }
    }
}

#[test]
fn each_answer_moves_to_the_middle_of_the_remaining_range_until_the_search_stops() {
    let table: Vec<(usize, SelfRating, Vec<SearchAnswer>, Vec<usize>)> = vec![
        // 8 rungs.
        (8, NewToThis, vec![r(), r(), r(), r()], vec![0, 4, 6, 7]),
        (8, NewToThis, vec![r(), w(), w(), r()], vec![0, 4, 2, 1]),
        (8, NewToThis, vec![r(), w(), r(), r()], vec![0, 4, 2, 3]),
        (8, NewToThis, vec![r(), s(), w(), s()], vec![0, 4, 2, 1]),
        (8, KnowTheBasics, vec![r(), r(), r(), r()], vec![2, 5, 6, 7]),
        (8, KnowTheBasics, vec![w(), w(), r(), r()], vec![2, 0, 1, 0]),
        (
            8,
            UseItRegularly,
            vec![r(), w(), r(), r()],
            vec![5, 6, 5, 6],
        ),
        (
            8,
            UseItRegularly,
            vec![r(), r(), w(), w()],
            vec![5, 6, 7, 6],
        ),
        (8, Expert, vec![w(), r(), w(), r()], vec![7, 3, 5, 4]),
        (8, Expert, vec![w(), s(), w(), s()], vec![7, 3, 1, 0]),
        // 10 rungs.
        (
            10,
            NewToThis,
            vec![r(), r(), r(), r(), r()],
            vec![0, 5, 7, 8, 9],
        ),
        (
            10,
            NewToThis,
            vec![r(), r(), r(), r(), w()],
            vec![0, 5, 7, 8, 9],
        ),
        (
            10,
            KnowTheBasics,
            vec![r(), w(), r(), s()],
            vec![3, 6, 4, 5],
        ),
        (
            10,
            KnowTheBasics,
            vec![w(), r(), r(), r()],
            vec![3, 1, 2, 3],
        ),
        (
            10,
            UseItRegularly,
            vec![w(), r(), r(), r()],
            vec![6, 2, 4, 5],
        ),
        (10, Expert, vec![r(), w(), r(), r()], vec![9, 8, 9, 8]),
        // 12 rungs.
        (
            12,
            NewToThis,
            vec![r(), r(), r(), r(), r()],
            vec![0, 6, 9, 10, 11],
        ),
        (
            12,
            NewToThis,
            vec![r(), w(), r(), r(), w()],
            vec![0, 6, 3, 4, 5],
        ),
        (
            12,
            KnowTheBasics,
            vec![r(), w(), w(), r()],
            vec![4, 8, 6, 5],
        ),
        (
            12,
            KnowTheBasics,
            vec![s(), r(), r(), r()],
            vec![4, 1, 2, 3],
        ),
        (
            12,
            UseItRegularly,
            vec![r(), r(), w(), r()],
            vec![7, 9, 10, 9],
        ),
        (
            12,
            Expert,
            vec![w(), w(), w(), r(), r()],
            vec![11, 5, 2, 0, 1],
        ),
        (
            12,
            Expert,
            vec![w(), r(), w(), r(), w()],
            vec![11, 5, 8, 6, 7],
        ),
    ];
    for (len, rating, answers, expected) in table {
        assert_eq!(
            walk(len, rating, &answers),
            expected,
            "{rating:?} on {len} rungs, {answers:?}"
        );
    }
}

/// Every answer sequence the search can be given, depth first, with the rungs
/// it asked.
fn every_sequence(
    len: usize,
    rating: SelfRating,
    answers: &mut Vec<SearchAnswer>,
    asked: &mut Vec<usize>,
    visit: &mut dyn FnMut(&[SearchAnswer], &[usize]),
) {
    if is_finished(len, rating, answers) {
        assert_eq!(next_rung(len, rating, answers), None);
        visit(answers, asked);
        return;
    }
    assert!(
        answers.len() < 6,
        "a seventh question for {rating:?} on {len} rungs after {answers:?}"
    );
    let rung = next_rung(len, rating, answers).expect("an unfinished search asks a rung");
    assert!(rung < len, "rung {rung} is off a ladder of {len}");
    for answer in [r(), w(), s()] {
        answers.push(answer);
        asked.push(rung);
        every_sequence(len, rating, answers, asked, visit);
        answers.pop();
        asked.pop();
    }
}

fn for_every_sequence(visit: &mut dyn FnMut(usize, SelfRating, &[SearchAnswer], &[usize])) {
    for len in 8..=12 {
        for rating in RATINGS {
            every_sequence(
                len,
                rating,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut |answers, asked| visit(len, rating, answers, asked),
            );
        }
    }
}

#[test]
fn every_answer_sequence_asks_four_to_six_questions() {
    let mut sequences = 0;
    for_every_sequence(&mut |len, rating, answers, _| {
        sequences += 1;
        assert!(
            (4..=6).contains(&answers.len()),
            "{} questions for {rating:?} on {len} rungs: {answers:?}",
            answers.len()
        );
    });
    assert!(sequences > 0);
}

#[test]
fn a_boundary_found_early_is_re_tested_until_four_questions() {
    let mut early = 0;
    for_every_sequence(&mut |len, rating, answers, asked| {
        // The first question after which two neighbouring asked rungs disagree.
        let known = |q: usize| matches!(answers[q], SearchAnswer::Right);
        let boundary = (1..asked.len()).find_map(|last| {
            (0..=last)
                .flat_map(|a| (0..=last).map(move |b| (a, b)))
                .find_map(|(a, b)| {
                    (asked[b] == asked[a] + 1 && known(a) && !known(b))
                        .then_some((asked[a], asked[b], last))
                })
        });
        let Some((low, high, last)) = boundary else {
            return;
        };
        if last > 2 {
            return;
        }
        early += 1;
        assert_eq!(
            asked.len(),
            4,
            "{rating:?} on {len} rungs: {answers:?} asked {asked:?}"
        );
        for q in last + 1..4 {
            assert!(
                asked[q] == low || asked[q] == high,
                "question {} is off the boundary {low}/{high}: {rating:?} on {len} rungs, asked {asked:?}",
                q + 1
            );
            assert_ne!(
                asked[q],
                asked[q - 1],
                "re-tests alternate: asked {asked:?}"
            );
        }
    });
    assert!(
        early > 0,
        "no sequence found a boundary on the second or third question"
    );
}

#[test]
fn a_boundary_on_the_second_question_re_tests_both_rungs() {
    // 8 rungs, use it regularly: right at 5, then wrong at its neighbour 6.
    assert_eq!(
        walk(8, UseItRegularly, &[r(), w(), r(), w()]),
        vec![5, 6, 5, 6]
    );
    assert_eq!(
        walk(8, UseItRegularly, &[r(), s(), s(), s()]),
        vec![5, 6, 5, 6]
    );
    // 10 rungs, know the basics: wrong at 3, right at 1, right at 2.
    assert_eq!(
        walk(10, KnowTheBasics, &[w(), r(), r(), w()]),
        vec![3, 1, 2, 3]
    );
}

#[test]
fn an_expert_right_at_the_top_re_tests_the_top_and_its_neighbour() {
    for len in [8, 10, 12] {
        let top = len - 1;
        for second in [r(), w(), s()] {
            for third in [r(), w(), s()] {
                for fourth in [r(), w(), s()] {
                    let answers = [r(), second.clone(), third.clone(), fourth];
                    assert_eq!(
                        walk(len, Expert, &answers),
                        vec![top, top - 1, top, top - 1],
                        "{len} rungs, {answers:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_newcomer_wrong_at_the_bottom_re_tests_the_bottom_and_its_neighbour() {
    for len in [8, 10, 12] {
        for first in [w(), s()] {
            for second in [r(), w(), s()] {
                for third in [r(), w(), s()] {
                    for fourth in [r(), w(), s()] {
                        let answers = [first.clone(), second.clone(), third.clone(), fourth];
                        assert_eq!(
                            walk(len, NewToThis, &answers),
                            vec![0, 1, 0, 1],
                            "{len} rungs, {answers:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn asked_rungs_are_known_or_unknown_and_the_rest_are_inferred() {
    // Asked 0, 4, 2, 3: 0 and 2 right, 4 wrong, 3 not sure.
    let answers = [r(), wm("four"), r(), s()];
    let placed = profile(&ladder(8), NewToThis, Goal::WorkingKnowledge, &answers);
    assert_eq!(placed.goal, Goal::WorkingKnowledge);
    assert_eq!(
        placed
            .rungs
            .iter()
            .map(|rung| rung.rung)
            .collect::<Vec<_>>(),
        (0..8).map(id).collect::<Vec<_>>()
    );
    assert_eq!(states(&placed), vec![K, K, K, U, U, U, U, U]);
    assert_eq!(placed.misconceptions, vec![misconception(4, "four")]);

    // Asked 11, 5, 8, 6, 7: 5 and 6 right, the others wrong.
    let answers = [wm("eleven"), r(), wm("eight"), r(), wm("seven")];
    let placed = profile(&ladder(12), Expert, Goal::Expert, &answers);
    assert_eq!(states(&placed), vec![K, K, K, K, K, K, K, U, U, U, U, U]);
    assert_eq!(
        placed.misconceptions,
        vec![
            misconception(11, "eleven"),
            misconception(8, "eight"),
            misconception(7, "seven"),
        ]
    );
}

#[test]
fn conflicting_answers_on_a_rung_make_it_shaky() {
    // Asked 3, 1, 2, 3: 3 wrong then right, 1 and 2 right. Nothing asked is
    // unknown, so the rungs above 3 are reached by neither rule.
    let answers = [wm("three"), r(), r(), r()];
    let placed = profile(&ladder(10), KnowTheBasics, Goal::Overview, &answers);
    assert_eq!(states(&placed), vec![K, K, K, Y, U, U, U, U, U, U]);
    assert_eq!(placed.misconceptions, vec![misconception(3, "three")]);

    // Asked 5, 6, 5, 6 with each rung answered both ways: nothing is known or
    // unknown, so every other rung is reached by neither rule.
    let answers = [r(), w(), w(), r()];
    let placed = profile(&ladder(8), UseItRegularly, Goal::Overview, &answers);
    assert_eq!(states(&placed), vec![U, U, U, U, U, Y, Y, U]);
    assert!(placed.misconceptions.is_empty());
}

#[test]
fn every_misconception_picked_is_recorded_against_its_rung() {
    // Asked 0, 1, 0, 1.
    let answers = [wm("a"), wm("b"), wm("c"), r()];
    let placed = profile(&ladder(8), NewToThis, Goal::Overview, &answers);
    assert_eq!(states(&placed), vec![U, Y, U, U, U, U, U, U]);
    assert_eq!(
        placed.misconceptions,
        vec![
            misconception(0, "a"),
            misconception(1, "b"),
            misconception(0, "c"),
        ]
    );
}

#[test]
fn the_step_asks_the_ladder_rung_until_the_search_is_done() {
    let ladder = ladder(8);
    assert_eq!(
        step(&ladder, KnowTheBasics, Goal::Overview, &[]),
        SearchStep::Ask(id(2))
    );
    assert_eq!(
        step(&ladder, KnowTheBasics, Goal::Overview, &[r()]),
        SearchStep::Ask(id(5))
    );
    let answers = [r(), r(), r(), r()];
    assert_eq!(
        step(&ladder, KnowTheBasics, Goal::Overview, &answers),
        SearchStep::Done(profile(&ladder, KnowTheBasics, Goal::Overview, &answers))
    );
}

fn one_rung(state: Knowledge) -> CandidateProfile {
    CandidateProfile {
        goal: Goal::Overview,
        rungs: vec![
            RungKnowledge {
                rung: id(0),
                state: Knowledge::Known,
            },
            RungKnowledge { rung: id(1), state },
        ],
        misconceptions: Vec::new(),
    }
}

#[test]
fn a_right_check_answer_moves_the_rung_one_step_toward_known() {
    for (before, after) in [(U, Y), (Y, K), (K, K)] {
        let mut profile = one_rung(before);
        apply_check(&mut profile, id(1), &r()).unwrap();
        assert_eq!(states(&profile), vec![K, after], "from {before:?}");
        assert!(profile.misconceptions.is_empty());
    }
}

#[test]
fn a_wrong_check_answer_moves_the_rung_one_step_toward_unknown_and_records_the_misconception() {
    for (before, after) in [(K, Y), (Y, U), (U, U)] {
        let mut profile = one_rung(before);
        apply_check(&mut profile, id(1), &wm("off by one")).unwrap();
        assert_eq!(states(&profile), vec![K, after], "from {before:?}");
        assert_eq!(profile.misconceptions, vec![misconception(1, "off by one")]);
    }
}

#[test]
fn a_check_on_a_rung_the_profile_lacks_is_refused() {
    let mut profile = one_rung(K);
    let before = profile.clone();
    assert_eq!(
        apply_check(&mut profile, id(5), &r()),
        Err(UnknownRung(id(5)))
    );
    assert_eq!(profile, before);
}
