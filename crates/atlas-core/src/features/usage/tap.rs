//! The event sink the other features emit through: every event goes on
//! unchanged, and a generation that finished completed then asks the meter
//! for a reading, taken off the emitting path.

use std::sync::{Arc, Weak};

use super::Meter;
use crate::ports::EventSink;
use crate::protocol::{CoreEvent, GenerationEvent, GenerationOutcome};

pub(super) struct Tap {
    pub(super) events: Arc<dyn EventSink>,
    pub(super) meter: Weak<Meter>,
}

impl EventSink for Tap {
    fn emit(&self, event: CoreEvent) {
        let completed = completes_a_generation(&event);
        self.events.emit(event);
        if completed && let Some(meter) = self.meter.upgrade() {
            meter.generation_completed();
        }
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
}
