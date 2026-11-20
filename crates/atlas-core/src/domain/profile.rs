//! The candidate profile: what Atlas believes the candidate knows. Never a
//! single score.

use serde::{Deserialize, Serialize};

use super::ladder::RungId;
use crate::protocol::Goal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Knowledge {
    Known,
    /// Conflicting answers.
    Shaky,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RungKnowledge {
    pub rung: RungId,
    pub state: Knowledge,
}

/// A wrong option the candidate picked, recorded against its rung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Misconception {
    pub rung: RungId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateProfile {
    pub goal: Goal,
    /// Every rung of the ladder, in ladder order.
    pub rungs: Vec<RungKnowledge>,
    pub misconceptions: Vec<Misconception>,
}
