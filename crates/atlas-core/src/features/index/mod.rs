//! Building a course's index (the broad pass) and its course map.
//!
//! For now the thin loop: the index is built straight from the topic, with no
//! placement and no map.

use std::sync::Arc;

use super::Features;
use crate::CoreDeps;
use crate::harness::index::IndexDraft;
use crate::harness::prompts;
use crate::protocol::{CoreError, CourseView};
use crate::provider::{ModelProvider, ProviderRequest, Task, complete_text};
use crate::store::Store;

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
        })
    }

    /// Stores a new course on `topic` with the index the provider builds.
    pub(crate) async fn build_from_topic(
        &self,
        cx: &Features,
        topic: &str,
    ) -> Result<CourseView, CoreError> {
        let text = complete_text(
            self.provider.as_ref(),
            ProviderRequest {
                model: cx.models.current(cx).await?,
                ..ProviderRequest::new(
                    Task::BuildIndex,
                    prompts::BUILD_INDEX.text(),
                    format!("Topic: {topic}"),
                )
            },
            |_| {},
        )
        .await?;
        let index: IndexDraft =
            serde_json::from_str(&text).map_err(|error| CoreError::Provider {
                message: format!("the index is not valid: {error}"),
            })?;
        if index.chapters.len() < 2 || index.chapters.iter().any(|c| c.sections.is_empty()) {
            return Err(CoreError::Provider {
                message: "the index needs at least two chapters, each with sections".to_owned(),
            });
        }
        Ok(self.store.insert_course(topic, &index)?)
    }
}
