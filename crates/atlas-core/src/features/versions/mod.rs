//! "Explain differently": new versions of a section in another style, and the
//! choice of the current version.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, ExplainStyle, SectionView};
use crate::store::Store;

const FEATURE: &str = "versions";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn explain_differently(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _section_id: Uuid,
        _style: ExplainStyle,
    ) -> Result<SectionView, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn select_version(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _section_id: Uuid,
        _version_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        Err(unavailable(FEATURE))
    }
}
