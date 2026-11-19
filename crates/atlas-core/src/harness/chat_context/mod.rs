//! The context a side-chat reply is written from: the passage, its section's
//! full text, the index titles, the profile and the history, with older turns
//! condensed over budget. It never holds the course memory.

use crate::protocol::ChatRole;

/// One turn of a side chat's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurn {
    pub role: ChatRole,
    pub text: String,
    /// A reply stopped before it finished.
    pub stopped: bool,
}
