#![forbid(unsafe_code)]

//! Current executable code and the metadata native entry planning actually reads.
//! Build companions, source record coordinates and source producer methods end
//! at the publication boundary.
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;

#[derive(Debug)]
pub struct PublishedProgram {
    source: std::sync::Weak<source_compiler::PortableTerminalProgram>,
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    evidence: Option<Arc<terminal_codec::EvidenceSeals>>,
    entry: Arc<PublishedEntryMetadata>,
    has_applications: bool,
}

pub use terminal_psi::PublishedEntryMetadata;

impl PublishedProgram {
    pub fn from_source(program: &Arc<source_compiler::PortableTerminalProgram>) -> Self {
        let artifact = Arc::clone(program.artifact());
        Self {
            source: Arc::downgrade(program),
            artifact,
            evidence: program.evidence().cloned(),
            has_applications: !program.applications().is_empty(),
            entry: Arc::clone(program.entry_metadata()),
        }
    }

    pub fn belongs_to(&self, source: &Arc<source_compiler::PortableTerminalProgram>) -> bool {
        self.source.ptr_eq(&Arc::downgrade(source))
    }

    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn evidence(&self) -> Option<&Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }
    pub fn has_applications(&self) -> bool {
        self.has_applications
    }
    pub fn entry_metadata(&self) -> &Arc<PublishedEntryMetadata> {
        &self.entry
    }
    pub fn machine_provenance(&self) -> &terminal_psi::TerminalMachineProvenance {
        self.entry.machine_provenance()
    }
    pub fn entry_schema_custody(
        &self,
        declaration: terminal_psi::TerminalIdentityIndex,
    ) -> Result<DeclarationCustody, String> {
        self.entry.entry_schema_custody(declaration)
    }
}

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

use diagnostics::Diagnostic;
use optimization::PsiOptimizationSelections;
use source_compiler::SourcePublication;
use source_compiler::{PortableTerminalProgram, PsiCompilationObserver};

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
