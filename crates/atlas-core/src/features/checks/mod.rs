//! Check questions: whether an answer is right, and the profile update the
//! core computes from it.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CheckResult, CoreError};
use crate::store::Store;

const FEATURE: &str = "checks";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn answer(
        &self,
        _cx: &Features,
        _version_id: Uuid,
        _block: u32,
        _question: u32,
        _choice: u32,
    ) -> Result<CheckResult, CoreError> {
        Err(unavailable(FEATURE))
    }
}
