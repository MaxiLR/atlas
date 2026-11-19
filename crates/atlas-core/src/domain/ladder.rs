//! The concept ladder: eight to twelve concepts of the topic in prerequisite
//! order, drafted before placement and never shown.

use serde::{Deserialize, Serialize};

/// A rung's id: its position on the ladder, from 1 at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RungId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rung {
    pub id: RungId,
    pub concept: String,
}

/// Rungs in prerequisite order, lowest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ladder {
    pub rungs: Vec<Rung>,
}
