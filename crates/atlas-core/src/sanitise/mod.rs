//! The allow-list sanitiser for figures the model writes: SVG parsed and
//! re-serialised with nothing that can run, fetch or escape the theme.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// SVG that passed the sanitiser. Inside the core it is made only by the
/// sanitiser; the interface receives it as a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
pub struct SanitisedSvg(String);

impl SanitisedSvg {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a figure was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the figure was rejected: {reason}")]
pub struct Rejected {
    pub reason: String,
}

/// Parses `input` as SVG and re-serialises what the allow-list keeps.
pub fn sanitise(_input: &str) -> Result<SanitisedSvg, Rejected> {
    Err(Rejected {
        reason: "the figure sanitiser is not built yet".to_owned(),
    })
}
