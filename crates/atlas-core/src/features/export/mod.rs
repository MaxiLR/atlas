//! PDF export: the course as it stands, read from the store only, printed
//! through the `PdfExporter` port.

use std::sync::Arc;

use uuid::Uuid;

use super::{Features, unavailable};
use crate::CoreDeps;
use crate::protocol::{CoreError, ExportDocument, ExportResult};
use crate::store::Store;

const FEATURE: &str = "export";

pub(crate) struct Feature;

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, _store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self)
    }

    pub(crate) async fn document(
        &self,
        _cx: &Features,
        _course_id: Uuid,
    ) -> Result<ExportDocument, CoreError> {
        Err(unavailable(FEATURE))
    }

    pub(crate) async fn export_pdf(
        &self,
        _cx: &Features,
        _course_id: Uuid,
    ) -> Result<ExportResult, CoreError> {
        Err(unavailable(FEATURE))
    }
}
