//! The model list of the plan route.

use crate::protocol::Model;
use crate::provider::ProviderError;

pub fn parse_models(_body: &str) -> Result<Vec<Model>, ProviderError> {
    Ok(Vec::new())
}
