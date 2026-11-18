//! The published program and the settled inputs for executable planning.
//!
//! This handoff contains no source tree, source publication job, provider
//! inventory or accepted Build source. Inspection and package review retain
//! their source through CheckedCompilation; executable planning cannot reopen it.

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct PublishedCompilation {
    pub(crate) root_grants: Arc<[provider_correspondence::trust::TrustAdmission]>,
    pub(crate) execution: crate::SelectedProviderResult,
    pub(crate) package_inputs: crate::ExecutablePackageInputs,
    pub(crate) program: Arc<PublishedExecutableProgram>,
    pub(crate) application: target::NativeApplicationMetadata,
    pub(crate) behavior_exclusions: Vec<effects::AuthoredBehaviorExclusion>,
    pub(crate) x86_deployment_claim:
        package_compilation::build_execution::build_value::X86DeploymentClaim,
}

impl PublishedCompilation {
    /// The obligations admitted for this target's exact selected providers.
    pub fn root_grants(&self) -> &[provider_correspondence::trust::TrustAdmission] {
        &self.root_grants
    }

    /// The completed provider realization for this executable publication.
    ///
    /// ```compile_fail,E0599
    /// fn replace_program(published: &products::PublishedCompilation, view: terminal_codec::TerminalArtifactView<'_>) {
    ///     let realized = published.execution().realization().clone();
    ///     let _ = realized.with_folded_program(view);
    /// }
    /// ```
    pub fn execution(&self) -> &crate::SelectedProviderResult {
        &self.execution
    }

    pub fn package_inputs(&self) -> &crate::ExecutablePackageInputs {
        &self.package_inputs
    }

    pub fn program(&self) -> &Arc<PublishedExecutableProgram> {
        &self.program
    }

    pub fn native_application_metadata(&self) -> target::NativeApplicationMetadata {
        self.application.clone()
    }

    pub fn behavior_exclusions(&self) -> &[effects::AuthoredBehaviorExclusion] {
        &self.behavior_exclusions
    }

    pub fn x86_deployment_claim(
        &self,
    ) -> package_compilation::build_execution::build_value::X86DeploymentClaim {
        self.x86_deployment_claim
    }
}

/// Source metadata identifies declarations and provenance. Executable code and
/// its evidence belong to the exact target-specific artifact, when folding is used.
#[derive(Debug)]
pub struct PublishedExecutableProgram {
    entry: Arc<portable_publication::PublishedEntryMetadata>,
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    evidence: Option<Arc<terminal_codec::EvidenceSeals>>,
}
impl PublishedExecutableProgram {
    pub(crate) fn for_execution(
        source: &crate::PublishedProgram,
        execution: &crate::SelectedProviderResult,
    ) -> Result<Self, String> {
        let artifact = Arc::clone(execution.effective_artifact(source.artifact())?);
        let evidence = match execution.folded_program() {
            Some(folded) => {
                if source.has_applications() {
                    return Err(
                        "folded constant custody requires an unbound portable product".into(),
                    );
                }
                if folded.evidence().map(|e| &e.seals) != source.evidence().map(|e| &e.seals) {
                    return Err("folded constant artifact changed its accepted evidence".into());
                }
                folded.evidence().cloned()
            }
            None => source.evidence().cloned(),
        };
        Ok(Self {
            entry: Arc::clone(source.entry_metadata()),
            artifact,
            evidence,
        })
    }

    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn evidence(&self) -> Option<&Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }
    pub fn machine_provenance(&self) -> &terminal_psi::TerminalMachineProvenance {
        self.entry.machine_provenance()
    }
    pub fn entry_schema_custody(
        &self,
        declaration: terminal_psi::TerminalIdentityIndex,
    ) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, String> {
        self.entry.entry_schema_custody(declaration)
    }
}
