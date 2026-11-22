//! The event sink the other features emit through: every event goes on
//! unchanged, then the meter hears what concerns it. A session that signed
//! in has the meter await its first reading; one that signed out empties
//! it; a generation that finished completed asks for a reading. Readings are
//! taken off the emitting path.

use std::sync::{Arc, Weak};

use super::Meter;
use crate::ports::EventSink;
use crate::protocol::{CoreEvent, GenerationEvent, GenerationOutcome, Session};

pub(super) struct Tap {
    pub(super) events: Arc<dyn EventSink>,
    pub(super) meter: Weak<Meter>,
}

impl EventSink for Tap {
    fn emit(&self, event: CoreEvent) {
        let heard = heard(&event);
        self.events.emit(event);
        let (Some(heard), Some(meter)) = (heard, self.meter.upgrade()) else {
            return;
        };
        match heard {
            Heard::SignedIn => meter.signed_in(),
            Heard::SignedOut => meter.signed_out(),
            Heard::Completed => meter.read_now(),
        }
    }
}

/// What the meter hears of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Heard {
    SignedIn,
    SignedOut,
    /// A generation finished completed.
    Completed,
}

fn heard(event: &CoreEvent) -> Option<Heard> {
    match event {
        CoreEvent::SessionChanged {
            session: Session::SignedIn { .. },
        } => Some(Heard::SignedIn),
        CoreEvent::SessionChanged {
            session: Session::SignedOut { .. },
        } => Some(Heard::SignedOut),
        event if completes_a_generation(event) => Some(Heard::Completed),
        _ => None,
    }
}

/// Whether `event` is a generation finishing completed; a stopped, failed or
/// incomplete one is not.
fn completes_a_generation(event: &CoreEvent) -> bool {
    matches!(
        event,
        CoreEvent::Generation {
            event: GenerationEvent::Finished {
                outcome: GenerationOutcome::Completed
            },
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::protocol::{CoreError, GenerationTarget};

    fn finished(outcome: GenerationOutcome) -> CoreEvent {
        CoreEvent::Generation {
            generation_id: Uuid::nil(),
            target: GenerationTarget::SideChat {
                course_id: Uuid::nil(),
                chat_id: Uuid::nil(),
            },
            event: GenerationEvent::Finished { outcome },
        }
    }

    #[test]
    fn only_a_completed_generation_asks_for_a_reading() {
        assert!(completes_a_generation(&finished(
            GenerationOutcome::Completed
        )));
        for outcome in [
            GenerationOutcome::Stopped,
            GenerationOutcome::Failed {
                error: CoreError::Offline,
            },
            GenerationOutcome::Incomplete { reason: None },
        ] {
            assert!(
                !completes_a_generation(&finished(outcome.clone())),
                "{outcome:?}"
            );
        }
        assert!(!completes_a_generation(&CoreEvent::Generation {
            generation_id: Uuid::nil(),
            target: GenerationTarget::SideChat {
                course_id: Uuid::nil(),
                chat_id: Uuid::nil(),
            },
            event: GenerationEvent::Started,
        }));
    }

    #[test]
    fn a_session_signing_in_is_not_yet_signed_in() {
        let session = |session| CoreEvent::SessionChanged { session };
        assert_eq!(heard(&session(Session::SigningIn)), None);
        assert_eq!(
            heard(&session(Session::SignedOut { reason: None })),
            Some(Heard::SignedOut)
        );
        assert_eq!(
            heard(&finished(GenerationOutcome::Completed)),
            Some(Heard::Completed)
        );
    }
}
