//! The model list of the plan route: `GET {api}/models`.

use serde_json::Value;

use crate::protocol::Model;
use crate::provider::ProviderError;

/// The models of a `GET {api}/models` body meant for display
/// (`visibility == "list"`), in the order reported. A body that is not a
/// model list, a listed entry without a string `slug` and `display_name`, or
/// a list with no listed model is an error.
pub fn parse_models(body: &str) -> Result<Vec<Model>, ProviderError> {
    let malformed = |what: &str| ProviderError::Failed(format!("the model list {what}"));
    let body: Value =
        serde_json::from_str(body).map_err(|error| malformed(&format!("is not JSON: {error}")))?;
    let entries = body
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("has no `models` array"))?;

    let models = entries
        .iter()
        .filter(|entry| entry.get("visibility").and_then(Value::as_str) == Some("list"))
        .map(|entry| {
            let field = |name: &str| {
                entry
                    .get(name)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| malformed(&format!("has a listed model without `{name}`")))
            };
            Ok(Model {
                slug: field("slug")?,
                display_name: field("display_name")?,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;

    if models.is_empty() {
        return Err(malformed("lists no model"));
    }
    Ok(models)
}
