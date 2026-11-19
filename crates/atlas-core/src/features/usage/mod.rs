//! The usage meter: its own sign-in, readings after generations and while the
//! window has focus, stale readings kept. Nothing here affects other requests.

use std::sync::Arc;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::ports::{EventSink, SecretRecord, SecretStore};
use crate::protocol::{CoreError, CoreEvent, UsageState};
use crate::store::Store;

const FEATURE: &str = "usage";

pub(crate) struct Feature {
    secrets: Arc<dyn SecretStore>,
    events: Arc<dyn EventSink>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            secrets: deps.secrets.clone(),
            events: deps.events.clone(),
        })
    }

    pub(crate) async fn get(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn connect(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn cancel_connect(&self, _cx: &Features) -> Result<UsageState, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn set_window_focused(
        &self,
        _cx: &Features,
        _focused: bool,
    ) -> Result<(), CoreError> {
        Err(unavailable(FEATURE))
    }

    /// Ends the usage login for the session's sign-out: its record is
    /// cleared and the meter is not connected.
    pub(crate) async fn sign_out(&self, _cx: &Features) -> Result<(), CoreError> {
        self.secrets
            .delete(SecretRecord::UsageLogin)
            .map_err(|error| CoreError::Store {
                message: error.to_string(),
            })?;
        self.events.emit(CoreEvent::UsageChanged {
            usage: UsageState::NotConnected,
        });
        Ok(())
    }
}
