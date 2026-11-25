//! The event sink the other features emit through, and the model provider
//! they stream through.
//!
//! Every event goes on unchanged, then the meter hears what concerns it. A
//! session that signed in has the meter await its first reading; one that
//! signed out empties it; a generation that started or finished tells
//! whether one is running, and one that finished completed asks for a
//! reading. Readings are taken off the emitting path.
//!
//! Every stream goes on unchanged too, and the meter hears the usage each
//! response reports, which is not a reading.

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use futures::StreamExt;
use uuid::Uuid;

use super::Meter;
use crate::ports::EventSink;
use crate::protocol::{CoreEvent, GenerationEvent, GenerationOutcome, Model, Session};
use crate::provider::{
    ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, TokenSource,
};

pub(super) struct Tap {
    pub(super) events: Arc<dyn EventSink>,
    pub(super) meter: Weak<Meter>,
}

impl EventSink for Tap {
    fn emit(&self, event: CoreEvent) {
        let heard = heard(&event);
        let running = running(&event);
        self.events.emit(event);
        if heard.is_none() && running.is_none() {
            return;
        }
        let Some(meter) = self.meter.upgrade() else {
            return;
        };
        if let Some((generation, running)) = running {
            meter.generation(generation, running);
        }
        match heard {
            Some(Heard::SignedIn) => meter.signed_in(),
            Some(Heard::SignedOut) => meter.signed_out(),
            Some(Heard::Completed) => meter.read_now(),
            None => {}
        }
    }
}

pub(super) struct ProviderTap {
    pub(super) provider: Arc<dyn ModelProvider>,
    pub(super) meter: Weak<Meter>,
}

#[async_trait]
impl ModelProvider for ProviderTap {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let stream = self.provider.stream(request).await?;
        let meter = self.meter.clone();
        Ok(Box::pin(stream.inspect(move |event| {
            if let Ok(ProviderEvent::Usage(snapshot)) = event
                && let Some(meter) = meter.upgrade()
            {
                meter.reported(snapshot);
            }
        })))
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        self.provider.list_models().await
    }

    fn use_plan_login(&self, tokens: Arc<dyn TokenSource>) -> bool {
        self.provider.use_plan_login(tokens)
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

/// The generation `event` starts or finishes, if it does: its id, and
/// whether it runs from now on.
fn running(event: &CoreEvent) -> Option<(Uuid, bool)> {
    match event {
        CoreEvent::Generation {
            generation_id,
            event: GenerationEvent::Started,
            ..
        } => Some((*generation_id, true)),
        CoreEvent::Generation {
            generation_id,
            event: GenerationEvent::Finished { .. },
            ..
        } => Some((*generation_id, false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn a_generation_runs_from_its_start_to_its_end_whatever_the_outcome() {
        let generation = Uuid::from_u128(7);
        let event = |event| CoreEvent::Generation {
            generation_id: generation,
            target: GenerationTarget::SideChat {
                course_id: Uuid::nil(),
                chat_id: Uuid::nil(),
            },
            event,
        };
        assert_eq!(
            running(&event(GenerationEvent::Started)),
            Some((generation, true))
        );
        for outcome in [
            GenerationOutcome::Completed,
            GenerationOutcome::Stopped,
            GenerationOutcome::Failed {
                error: CoreError::Offline,
            },
            GenerationOutcome::Incomplete { reason: None },
        ] {
            assert_eq!(
                running(&event(GenerationEvent::Finished {
                    outcome: outcome.clone()
                })),
                Some((generation, false)),
                "{outcome:?}"
            );
        }
        assert_eq!(
            running(&event(GenerationEvent::TextDelta {
                block: 0,
                text: "text".to_owned()
            })),
            None
        );
    }
}
