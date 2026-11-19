//! Links opened in the system browser: ChatGPT's usage settings, and `http`
//! or `https` links from the interface.

use std::sync::Arc;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::CoreError;
use crate::store::Store;

const FEATURE: &str = "external";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn open_manage_usage(&self, _cx: &Features) -> Result<(), CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn open_external(
        &self,
        _cx: &Features,
        _url: String,
    ) -> Result<(), CoreError> {
        Err(unavailable(FEATURE))
    }
}
