//! The model picker: the models the plan lists, the default, and the
//! candidate's pick while it is still listed.

use std::sync::Arc;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, ModelList};
use crate::store::Store;

const FEATURE: &str = "models";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn list(&self, _cx: &Features) -> Result<ModelList, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn select(
        &self,
        _cx: &Features,
        _slug: String,
    ) -> Result<ModelList, CoreError> {
        Err(unavailable(FEATURE))
    }
}
