use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A model the plan lists for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub slug: String,
    pub display_name: String,
}

/// The models in the order the plan reports them, and the slug in use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelList {
    pub models: Vec<Model>,
    pub current: Option<String>,
}
