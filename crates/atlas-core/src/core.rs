use std::path::PathBuf;
use std::sync::Arc;

use crate::ports::EventSink;
use crate::protocol::{CoreError, CoreRequest, CoreResponse};
use crate::provider::ModelProvider;

/// Everything the core is built from.
pub struct CoreDeps {
    /// The SQLite database file; created if missing.
    pub database_path: PathBuf,
    pub provider: Arc<dyn ModelProvider>,
    pub events: Arc<dyn EventSink>,
}

/// The core's single entry point: one typed request in, one typed response
/// out, events on the side.
pub struct Core {
    _deps: CoreDeps,
}

impl Core {
    pub fn new(deps: CoreDeps) -> Result<Self, CoreError> {
        Ok(Self { _deps: deps })
    }

    pub async fn handle(&self, _request: CoreRequest) -> Result<CoreResponse, CoreError> {
        Err(CoreError::Unavailable {
            feature: "the thin loop".to_owned(),
        })
    }
}
