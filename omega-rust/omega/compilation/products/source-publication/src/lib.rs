#![forbid(unsafe_code)]

//! Join the Psi publication to the package inputs selected for this product.
pub use checked_terminal_psi_to_published_psi::PortableTerminalProgram as PublishedProgram;
use std::sync::Arc;
pub use terminal_psi::PublishedEntryMetadata;

#[derive(Debug, Clone)]
pub struct ExecutablePackageInputs {
    declaration_permissions:
        Option<semantic_vocabulary::compiler_evaluation::ProductDeclarationScope>,
    accepted_semantic_bindings: Vec<package_compilation::AcceptedSemanticBinding>,
}

impl ExecutablePackageInputs {
    pub fn from_checked(sources: &package_compilation::CheckedSourceCustody) -> Self {
        Self {
            declaration_permissions: sources
                .dependency_closure()
                .map(|graph| graph.declaration_permissions()),
            accepted_semantic_bindings: sources.accepted_semantic_bindings().to_vec(),
        }
    }
    pub fn declaration_permissions(
        &self,
    ) -> Option<&semantic_vocabulary::compiler_evaluation::ProductDeclarationScope> {
        self.declaration_permissions.as_ref()
    }
    pub fn accepted_semantic_bindings(&self) -> &[package_compilation::AcceptedSemanticBinding] {
        &self.accepted_semantic_bindings
    }

    pub fn accepted_semantic_binding(
        &self,
        role: package_compilation::AcceptedSemanticBindingRole,
    ) -> Option<&package_compilation::AcceptedSemanticBinding> {
        self.accepted_semantic_bindings
            .iter()
            .find(|binding| binding.role() == role)
    }
}

// Every product consumes the same accepted source and its published module.
// No old producer can replace a failed publication or a different root selection.

use checked_terminal_psi_to_published_psi::SourcePublication;
use checked_terminal_psi_to_published_psi::{PortableTerminalProgram, PsiCompilationObserver};
use diagnostics::Diagnostic;
use optimization::PsiOptimizationSelections;

#[derive(Clone, Copy)]
pub struct ProductCodeSource<'a> {
    pub publication: &'a SourcePublication,
    pub sources: &'a package_compilation::CheckedSourceCustody,
}

impl<'a> ProductCodeSource<'a> {
    pub fn publish(
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

mod provider_demand;
pub use provider_demand::{PortableDemandIssuer, PublishedProviderDemand};
