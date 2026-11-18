//! Invocation-local admitted source catalogue and reusable parsing frontends.

use diagnostics::Diagnostic;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct SourcePreparation {
    catalogue: Arc<Mutex<crate::source_inputs::SourceCatalogue>>,
    pub frontends: crate::SourceFrontends,
}

impl SourcePreparation {
    pub fn catalogue(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, crate::source_inputs::SourceCatalogue>, Vec<Diagnostic>>
    {
        self.catalogue
            .lock()
            .map_err(|_| vec![Diagnostic::error("source catalogue owner is poisoned")])
    }
}
