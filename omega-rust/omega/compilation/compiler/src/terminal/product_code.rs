// Every product consumes the same accepted source and its published module.
// No old producer can replace a failed publication or a different root selection.

use compilation_products::{CheckedProductSource, ProductTerminalPublication};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use psi_compiler::{PortableTerminalProgram, PsiCompilationObserver};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct ProductCodeSource<'a> {
    pub publication: &'a ProductTerminalPublication,
}

impl<'a> ProductCodeSource<'a> {
    pub(crate) fn checked(source: &'a CheckedProductSource) -> Self {
        Self {
            publication: &source.terminal_publication,
        }
    }

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

    pub(crate) fn inspect(
        &self,
        _name: &str,
        _selections: &OptimizationSelections,
        _observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        Err(vec![Diagnostic::error(
            "no product route yet: named inspection requires its own accepted source selection",
        )])
    }
}
