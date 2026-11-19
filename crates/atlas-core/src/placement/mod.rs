//! The placement search over the concept ladder (ADR-0006): which rung to ask
//! next, when to stop, and the profile that results.

use crate::domain::ladder::{Ladder, RungId};
use crate::domain::profile::CandidateProfile;
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

pub fn next_rung(
    _ladder_len: usize,
    _rating: SelfRating,
    _answers: &[SearchAnswer],
) -> Option<usize> {
    unimplemented!()
}

pub fn is_finished(_ladder_len: usize, _rating: SelfRating, _answers: &[SearchAnswer]) -> bool {
    unimplemented!()
}

pub fn step(
    _ladder: &Ladder,
    _rating: SelfRating,
    _goal: Goal,
    _answers: &[SearchAnswer],
) -> SearchStep {
    unimplemented!()
}

pub fn profile(
    _ladder: &Ladder,
    _rating: SelfRating,
    _goal: Goal,
    _answers: &[SearchAnswer],
) -> CandidateProfile {
    unimplemented!()
}

pub fn apply_check(
    _profile: &mut CandidateProfile,
    _rung: RungId,
    _answer: &SearchAnswer,
) -> Result<(), UnknownRung> {
    unimplemented!()
}
