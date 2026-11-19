//! The Wikipedia summary shown under a definition, stored with its entry.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, WikipediaResult};
use crate::store::Store;

const FEATURE: &str = "wikipedia";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn lookup(
        &self,
        _cx: &Features,
        _entry_id: Uuid,
    ) -> Result<WikipediaResult, CoreError> {
        Err(unavailable(FEATURE))
    }
}
