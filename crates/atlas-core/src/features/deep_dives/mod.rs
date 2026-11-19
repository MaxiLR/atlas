//! "Go deeper": three to six child sections under a section, extending the
//! course map.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, SectionNode};
use crate::store::Store;

const FEATURE: &str = "deep_dives";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn go_deeper(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _section_id: Uuid,
    ) -> Result<Vec<SectionNode>, CoreError> {
        Err(unavailable(FEATURE))
    }
}
