// Every product consumes the same accepted source and its published module.
// No old producer can replace a failed publication or a different root selection.

use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use products::ProductTerminalPublication;
use source_compiler::{PortableTerminalProgram, PsiCompilationObserver};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct ProductCodeSource<'a> {
    pub publication: &'a ProductTerminalPublication,
}

impl<'a> ProductCodeSource<'a> {
    pub(crate) fn publish(
        &self,
        selections: &OptimizationSelections,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        self.publication.retain(selections, || {
            self.publication
                .accepted()?
                .produce(Default::default(), observer)?
                .into_shared_portable()
                .map(Arc::new)
                .map_err(|error| vec![Diagnostic::error(error.to_string())])
        })
    }
}
