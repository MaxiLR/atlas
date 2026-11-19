//! One folder per feature, each with its own state built from the deps and
//! one method per request it answers. `core.rs` routes every request to its
//! feature once and for all, so a feature lands by editing its own folder.
//!
//! A feature not built yet answers [`CoreError::Unavailable`] naming itself.
//! Features reach each other through the [`Features`] passed to every method.

pub(crate) mod checks;
pub(crate) mod deep_dives;
pub(crate) mod define;
pub(crate) mod export;
pub(crate) mod external;
pub(crate) mod index;
pub(crate) mod library;
pub(crate) mod models;
pub(crate) mod navigation;
pub(crate) mod placement;
pub(crate) mod sections;
pub(crate) mod session;
pub(crate) mod side_chats;
pub(crate) mod usage;
pub(crate) mod versions;
pub(crate) mod wikipedia;

use std::sync::Arc;

use crate::CoreDeps;
use crate::protocol::CoreError;
use crate::store::Store;

pub(crate) struct Features {
    pub(crate) session: session::Feature,
    pub(crate) external: external::Feature,
    /// Shared with the session, which reports each session change to it.
    pub(crate) models: Arc<models::Feature>,
    pub(crate) library: library::Feature,
    pub(crate) navigation: navigation::Feature,
    pub(crate) placement: placement::Feature,
    pub(crate) index: index::Feature,
    pub(crate) sections: sections::Feature,
    pub(crate) versions: versions::Feature,
    pub(crate) checks: checks::Feature,
    pub(crate) deep_dives: deep_dives::Feature,
    pub(crate) define: define::Feature,
    pub(crate) wikipedia: wikipedia::Feature,
    pub(crate) side_chats: side_chats::Feature,
    pub(crate) usage: usage::Feature,
    pub(crate) export: export::Feature,
}

impl Features {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        let models = Arc::new(models::Feature::new(deps, store)?);
        Ok(Self {
            session: session::Feature::new(deps, store, &models)?,
            external: external::Feature::new(deps, store)?,
            models,
            library: library::Feature::new(deps, store)?,
            navigation: navigation::Feature::new(deps, store)?,
            placement: placement::Feature::new(deps, store)?,
            index: index::Feature::new(deps, store)?,
            sections: sections::Feature::new(deps, store)?,
            versions: versions::Feature::new(deps, store)?,
            checks: checks::Feature::new(deps, store)?,
            deep_dives: deep_dives::Feature::new(deps, store)?,
            define: define::Feature::new(deps, store)?,
            wikipedia: wikipedia::Feature::new(deps, store)?,
            side_chats: side_chats::Feature::new(deps, store)?,
            usage: usage::Feature::new(deps, store)?,
            export: export::Feature::new(deps, store)?,
        })
    }
}

/// The answer of a feature that is not built yet.
pub(crate) fn unavailable(feature: &str) -> CoreError {
    CoreError::Unavailable {
        feature: feature.to_owned(),
    }
}
