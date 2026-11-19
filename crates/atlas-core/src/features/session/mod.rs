//! The plan login: sign in, plan usage on, off or not allowed, sign out, the
//! `TokenSource` the provider draws from, and the `require_plan` guard.

use std::sync::Arc;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, Session};
use crate::store::Store;

const FEATURE: &str = "session";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    /// Refuses a request that spends plan usage unless plan usage is on.
    /// Passes until sign-in lands.
    pub(crate) async fn require_plan(&self, _cx: &Features) -> Result<(), CoreError> {
        Ok(())
    }

    pub(crate) async fn get(&self, _cx: &Features) -> Result<Session, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn sign_in(&self, _cx: &Features) -> Result<Session, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn cancel_sign_in(&self, _cx: &Features) -> Result<Session, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn enable_plan_usage(&self, _cx: &Features) -> Result<Session, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn sign_out(&self, _cx: &Features) -> Result<Session, CoreError> {
        Err(unavailable(FEATURE))
    }
}
