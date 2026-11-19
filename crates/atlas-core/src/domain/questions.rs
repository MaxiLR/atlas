//! Questions as the core keeps them, with their keys. The interface sees them
//! through `protocol::PlacementQuestion` and `protocol::CheckQuestion`, which
//! leave the key out (a check question's is told once it is answered).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ladder::RungId;

/// An option, labelled with the misconception it represents when it is wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyedOption {
    pub text: String,
    pub misconception: Option<String>,
}

/// A placement question tied to one rung: four options, one right.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyedPlacementQuestion {
    pub id: Uuid,
    pub rung: RungId,
    pub stem: String,
    pub options: Vec<KeyedOption>,
    /// The right option, counted from 0.
    pub key: u32,
}

/// A check question at the end of a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyedCheckQuestion {
    pub rung: RungId,
    pub stem: String,
    pub options: Vec<KeyedOption>,
    pub key: u32,
    pub explanation: String,
}
