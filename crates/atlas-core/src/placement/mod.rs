//! The placement search over the concept ladder (ADR-0006): which rung to ask
//! next, when to stop, and the profile that results.

use crate::domain::ladder::RungId;
use crate::domain::profile::CandidateProfile;

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
