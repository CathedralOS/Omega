//! Current executable code and the metadata native entry planning actually reads.
//! Build companions, source record coordinates and source producer methods end
//! at CheckedCompilation::into_published.
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use std::sync::Arc;

#[derive(Debug)]
pub struct PublishedProgram {
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    evidence: Option<Arc<terminal_codec::EvidenceSeals>>,
    provenance: terminal_psi::TerminalMachineProvenance,
    entry_custody: Vec<(
        terminal_psi::TerminalIdentityIndex,
        Result<DeclarationCustody, String>,
    )>,
}

impl PublishedProgram {
    pub(crate) fn from_source(program: &source_compiler::PortableTerminalProgram) -> Self {
        let artifact = Arc::clone(program.artifact());
        let module = artifact.semantic_module();
        let mut entry_custody = Vec::new();
        for entry in &module.program_entry_contracts {
            let declaration = entry.schema_declaration;
            if entry_custody.iter().any(|(known, _)| *known == declaration) {
                continue;
            }
            let custody = program
                .source_selections()
                .ok_or_else(|| {
                    "entry schema has no authenticated source selection custody".to_owned()
                })
                .and_then(|selections| {
                    let declaration = module.identities.text(declaration).ok_or_else(|| {
                        "entry schema lost its canonical declaration identity".to_owned()
                    })?;
                    selections.declaration_custody(
                        artifact.view().into(),
                        declaration,
                        semantic_vocabulary::source_selections::SelectionPurpose::Product,
                    )
                });
            entry_custody.push((declaration, custody));
        }

        Self {
            artifact,
            evidence: program.evidence().cloned(),
            provenance: program.machine_provenance().clone(),
            entry_custody,
        }
    }

    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn evidence(&self) -> Option<&Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }
    pub fn machine_provenance(&self) -> &terminal_psi::TerminalMachineProvenance {
        &self.provenance
    }
    pub fn entry_schema_custody(
        &self,
        declaration: terminal_psi::TerminalIdentityIndex,
    ) -> Result<DeclarationCustody, String> {
        self.entry_custody
            .iter()
            .find(|(identity, _)| *identity == declaration)
            .ok_or_else(|| "selected declaration has no authenticated source custody".to_owned())?
            .1
            .clone()
    }
}

#[derive(Debug, Clone)]
pub struct ExecutablePackageInputs {
    dependency_closure: Option<package_compilation::PackageDependencyClosure>,
    accepted_semantic_bindings: Vec<package_compilation::AcceptedSemanticBinding>,
}

impl ExecutablePackageInputs {
    pub(crate) fn from_checked(sources: &package_compilation::CheckedSourceCustody) -> Self {
        Self {
            dependency_closure: sources.dependency_closure().cloned(),
            accepted_semantic_bindings: sources.accepted_semantic_bindings().to_vec(),
        }
    }
    pub fn dependency_closure(&self) -> Option<&package_compilation::PackageDependencyClosure> {
        self.dependency_closure.as_ref()
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
