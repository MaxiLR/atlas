//! Side chats beside the lesson: anchored to a passage or to the course,
//! never part of section generation.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{Anchor, ChatMessage, CoreError, SideChat, SideChatSummary};
use crate::store::Store;

const FEATURE: &str = "side_chats";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn list(
        &self,
        _cx: &Features,
        _course_id: Uuid,
    ) -> Result<Vec<SideChatSummary>, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn start(
        &self,
        _cx: &Features,
        _course_id: Uuid,
        _anchor: Option<Anchor>,
    ) -> Result<SideChat, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn open(&self, _cx: &Features, _chat_id: Uuid) -> Result<SideChat, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn send(
        &self,
        _cx: &Features,
        _chat_id: Uuid,
        _text: String,
    ) -> Result<ChatMessage, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn stop_reply(&self, _cx: &Features, _chat_id: Uuid) -> Result<(), CoreError> {
        Err(unavailable(FEATURE))
    }
}
