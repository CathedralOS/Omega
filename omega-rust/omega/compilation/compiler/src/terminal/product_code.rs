// Every product consumes the same accepted source and its published module.
// No old producer can replace a failed publication or a different root selection.

use compilation_products::ProductTerminalPublication;
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use psi_compiler::{PortableTerminalProgram, PsiCompilationObserver};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct ProductCodeSource<'a> {
    pub publication: &'a ProductTerminalPublication,
    pub sources: &'a package_compilation::CheckedSourceCustody,
}

impl<'a> ProductCodeSource<'a> {
    pub(crate) fn publish(
        &self,
        selections: &OptimizationSelections,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        self.publication.retain(selections, || {
            let accepted = self.publication.accepted()?;
            self.sources
                .require_owner(accepted.source())
                .map_err(|error| vec![error])?;
            let program = accepted
                .produce(Default::default(), observer)?
                .into_shared_portable()
                .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
            let packet = program.source_selections().ok_or_else(|| {
                vec![Diagnostic::error(
                    "Product publication lost its source declaration selections",
                )]
            })?;
            let authority = self.sources.dependency_closure().map(|graph| {
                graph as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
            });
            packet
                .admit(
                    program.artifact().view().into(),
                    &terminal_installation::selection_questions::SelectionAuthorities {
                        build: None,
                        product: authority,
                    },
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            Ok(Arc::new(program))
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
