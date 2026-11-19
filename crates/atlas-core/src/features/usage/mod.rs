//! The usage meter: its own sign-in, readings after generations and while the
//! window has focus, stale readings kept. Nothing here affects other requests.

use std::sync::Arc;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, UsageState};
use crate::store::Store;

const FEATURE: &str = "usage";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
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
}
