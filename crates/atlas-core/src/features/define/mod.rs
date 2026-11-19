//! Define: a short explanation of selected text, kept in the course glossary.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{Anchor, CoreError, GlossaryEntry};
use crate::store::Store;

const FEATURE: &str = "define";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn define(
        &self,
        _cx: &Features,
        _anchor: Anchor,
        _sentence: String,
    ) -> Result<GlossaryEntry, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn list_glossary(
        &self,
        _cx: &Features,
        _course_id: Uuid,
    ) -> Result<Vec<GlossaryEntry>, CoreError> {
        Err(unavailable(FEATURE))
    }
}
