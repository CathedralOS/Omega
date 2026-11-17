// Every product consumes the same accepted source and its published module.
// No old producer can replace a failed publication or a different root selection.

use diagnostics::Diagnostic;
use optimization::PsiOptimizationSelections;
use source_compiler::SourcePublication;
use source_compiler::{PortableTerminalProgram, PsiCompilationObserver};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) struct ProductCodeSource<'a> {
    pub publication: &'a SourcePublication,
    pub sources: &'a package_compilation::CheckedSourceCustody,
}

impl<'a> ProductCodeSource<'a> {
    pub(crate) fn publish(
        &self,
        selections: &PsiOptimizationSelections,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Arc<PortableTerminalProgram>, Vec<Diagnostic>> {
        self.sources
            .require_owner(self.publication.accepted()?.source())
            .map_err(|error| vec![error])?;
        let authority = self.sources.dependency_closure().map(|graph| {
            graph as &dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority
        });
        self.publication.publish(selections, authority, observer)
    }
}
