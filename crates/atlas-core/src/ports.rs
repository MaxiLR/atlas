//! What the core needs from its host, injected through [`crate::CoreDeps`].

use crate::protocol::CoreEvent;

/// Receives the core's events; the shell forwards them to the interface.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: CoreEvent);
}
