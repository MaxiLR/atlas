//! The placement search over the concept ladder (ADR-0006): which rung to ask
//! next, when to stop, the profile that results, and how a check question
//! corrects it. Pure functions: no model, store or clock.

use crate::domain::ladder::{Ladder, RungId};
use crate::domain::profile::{CandidateProfile, Knowledge, Misconception, RungKnowledge};
use crate::protocol::{Goal, SelfRating};

/// An answer as the search sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAnswer {
    Right,
    /// A wrong option, with the misconception it represents.
    Wrong {
        misconception: Option<String>,
    },
    NotSure,
}

/// What the search does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStep {
    Ask(RungId),
    Done(CandidateProfile),
}

/// A check answer named a rung the profile does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("rung {} is not on the profile", .0.0)]
pub struct UnknownRung(pub RungId);

/// The fewest and the most questions placement asks.
pub const MIN_QUESTIONS: usize = 4;
pub const MAX_QUESTIONS: usize = 6;

/// The rung, counted from 0 at the most basic, of the question to ask after
/// `answers`, or `None` once the search is finished. `ladder_len` is between
/// two and 64.
pub fn next_rung(ladder_len: usize, rating: SelfRating, answers: &[SearchAnswer]) -> Option<usize> {
    replay(ladder_len, rating, answers, |_, _| {}).next
}

pub fn is_finished(ladder_len: usize, rating: SelfRating, answers: &[SearchAnswer]) -> bool {
    next_rung(ladder_len, rating, answers).is_none()
}

/// The question to ask next on `ladder`, or the profile once the search is
/// finished.
pub fn step(
    ladder: &Ladder,
    rating: SelfRating,
    goal: Goal,
    answers: &[SearchAnswer],
) -> SearchStep {
    match next_rung(ladder.rungs.len(), rating, answers) {
        Some(rung) => SearchStep::Ask(ladder.rungs[rung].id),
        None => SearchStep::Done(profile(ladder, rating, goal, answers)),
    }
}

/// The candidate profile the answers give. An asked rung is known when every
/// answer on it was right, unknown when none was, shaky otherwise. A rung not
/// asked is known below the highest known rung and unknown everywhere else.
/// Answers past the end of the search are not counted.
pub fn profile(
    ladder: &Ladder,
    rating: SelfRating,
    goal: Goal,
    answers: &[SearchAnswer],
) -> CandidateProfile {
    let mut misconceptions = Vec::new();
    let search = replay(ladder.rungs.len(), rating, answers, |rung, answer| {
        if let SearchAnswer::Wrong {
            misconception: Some(text),
        } = answer
        {
            misconceptions.push(Misconception {
                rung: ladder.rungs[rung].id,
                text: text.clone(),
            });
        }
    });
    let asked = |rung: usize| match (search.right(rung), search.not_right(rung)) {
        (true, false) => Some(Knowledge::Known),
        (true, true) => Some(Knowledge::Shaky),
        (false, true) => Some(Knowledge::Unknown),
        (false, false) => None,
    };
    let highest_known =
        (0..ladder.rungs.len()).rfind(|&rung| asked(rung) == Some(Knowledge::Known));
    let rungs = ladder
        .rungs
        .iter()
        .enumerate()
        .map(|(index, rung)| RungKnowledge {
            rung: rung.id,
            state: asked(index).unwrap_or(if highest_known.is_some_and(|known| index < known) {
                Knowledge::Known
            } else {
                Knowledge::Unknown
            }),
        })
        .collect();
    CandidateProfile {
        goal,
        rungs,
        misconceptions,
    }
}

/// A check question's answer on `rung`: right moves it one step toward known,
/// wrong or not sure one step toward unknown, and a wrong answer's
/// misconception is recorded.
pub fn apply_check(
    profile: &mut CandidateProfile,
    rung: RungId,
    answer: &SearchAnswer,
) -> Result<(), UnknownRung> {
    let entry = profile
        .rungs
        .iter_mut()
        .find(|entry| entry.rung == rung)
        .ok_or(UnknownRung(rung))?;
    entry.state = match (answer, entry.state) {
        (SearchAnswer::Right, Knowledge::Unknown) => Knowledge::Shaky,
        (SearchAnswer::Right, _) => Knowledge::Known,
        (_, Knowledge::Known) => Knowledge::Shaky,
        (_, _) => Knowledge::Unknown,
    };
    if let SearchAnswer::Wrong {
        misconception: Some(text),
    } = answer
    {
        profile.misconceptions.push(Misconception {
            rung,
            text: text.clone(),
        });
    }
    Ok(())
}

/// The rung the self-rating starts at: the lowest, a third of the way up, two
/// thirds of the way up, or the highest, rounded to the nearest rung.
fn first_rung(ladder_len: usize, rating: SelfRating) -> usize {
    let top = ladder_len - 1;
    match rating {
        SelfRating::NewToThis => 0,
        // round(top / 3) and round(2 * top / 3); a third never ends in a half.
        SelfRating::KnowTheBasics => (2 * top + 3) / 6,
        SelfRating::UseItRegularly => (4 * top + 3) / 6,
        SelfRating::Expert => top,
    }
}

/// Where the search stands after the answers it used.
struct Search {
    /// The rung to ask next, `None` once the search is finished.
    next: Option<usize>,
    /// Bit `i` is set when rung `i` had a right answer.
    right: u64,
    /// Bit `i` is set when rung `i` had a wrong or not sure answer.
    not_right: u64,
}

impl Search {
    fn right(&self, rung: usize) -> bool {
        self.right & (1 << rung) != 0
    }

    fn not_right(&self, rung: usize) -> bool {
        self.not_right & (1 << rung) != 0
    }
}

/// The two rungs re-tested in turn once the search has placed every rung.
#[derive(Clone, Copy)]
struct Retest {
    pair: (usize, usize),
    /// The end rung the search ran off and its neighbour, rather than two
    /// neighbours that disagree; given up for such a boundary as soon as a
    /// re-test reveals one.
    end: bool,
}

/// Runs the search over `answers`, calling `visit` with the rung each one
/// answered; answers past the end of the search are not visited.
///
/// The search narrows the range of rungs not yet placed: right moves above the
/// rung, wrong or not sure below it, and the next question is at the middle of
/// what remains. Once the range is empty, the two rungs at its edge (one
/// known, the next not, or the end rung and its neighbour when the search ran
/// off the ladder) are re-tested in turn until the minimum is reached.
fn replay(
    ladder_len: usize,
    rating: SelfRating,
    answers: &[SearchAnswer],
    mut visit: impl FnMut(usize, &SearchAnswer),
) -> Search {
    debug_assert!(
        (2..=64).contains(&ladder_len),
        "a ladder of {ladder_len} rungs"
    );
    let finished = |asked: usize, retest: Option<Retest>| {
        asked >= MAX_QUESTIONS || (retest.is_some() && asked >= MIN_QUESTIONS)
    };
    let mut search = Search {
        next: Some(first_rung(ladder_len, rating)),
        right: 0,
        not_right: 0,
    };
    // The rungs not yet placed.
    let (mut low, mut high) = (0, ladder_len);
    let mut retest: Option<Retest> = None;
    for (asked, answer) in answers.iter().enumerate() {
        let Some(rung) = search.next.take() else {
            break;
        };
        visit(rung, answer);
        let is_right = *answer == SearchAnswer::Right;
        if is_right {
            search.right |= 1 << rung;
        } else {
            search.not_right |= 1 << rung;
        }
        match retest {
            Some(Retest { end: false, .. }) => {}
            Some(Retest { end: true, .. }) => {
                // The answer may make this rung disagree with a neighbour.
                let boundary = if is_right {
                    (rung + 1 < ladder_len && search.not_right(rung + 1)).then(|| (rung, rung + 1))
                } else {
                    (rung > 0 && search.right(rung - 1)).then(|| (rung - 1, rung))
                };
                if let Some(pair) = boundary {
                    retest = Some(Retest { pair, end: false });
                }
            }
            None => {
                if is_right {
                    low = rung + 1;
                } else {
                    high = rung;
                }
                retest = if low < high {
                    None
                } else if low == ladder_len {
                    Some(Retest {
                        pair: (ladder_len - 1, ladder_len - 2),
                        end: true,
                    })
                } else if high == 0 {
                    Some(Retest {
                        pair: (0, 1),
                        end: true,
                    })
                } else {
                    Some(Retest {
                        pair: (low - 1, low),
                        end: false,
                    })
                };
            }
        }
        if finished(asked + 1, retest) {
            break;
        }
        search.next = Some(match retest {
            None => low + (high - low - 1) / 2,
            Some(Retest { pair: (a, b), .. }) => {
                if rung == a {
                    b
                } else {
                    a
                }
            }
        });
    }
    search
}
