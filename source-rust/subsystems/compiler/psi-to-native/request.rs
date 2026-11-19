use verified_optimization_input::{RetainedTerminalArtifact, TerminalNativeRealizationProposal};
use diagnostics::Diagnostic;
use optimization_core::OptimizationWorkBudget;
use provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};
use target::ExecutableImageEmissionRequest;

/// Per-pass traversal and validation ceilings for the reference extent (2^16 units).
pub(crate) fn native_optimization_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 100_000, 4_000_000, 100_000, 1_000_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}

pub struct NativeCompileRequest<'request> {
    /// Retain original stage inputs for explicitly requested correspondence audits.
    pub retain_native_stage_inputs: bool,
    pub private_entry_ram: &'request [psi_entry_memory::PrivateEntryRamInitialization],
    pub installation_assumptions:
        &'request [installation_conditions::InstallationEnvironmentAssumption],
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub image_request: ExecutableImageEmissionRequest,
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
    pub components: &'request [super::components::ComponentCompilation<'request>],
    /// Completed scalar values read by the program entry.
    pub scalars: Option<&'request psi_scalar_applications::AdmittedScalarApplications>,
}

impl NativeCompileRequest<'_> {
    pub(crate) fn prepare_terminal<'input>(
        &mut self,
        retained: &'input RetainedTerminalArtifact,
    ) -> Result<
        (
            &'input std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
            &'input TerminalNativeRealizationProposal,
        ),
        Vec<Diagnostic>,
    > {
        let proposal = retained.native_realization_proposal().ok_or_else(|| {
            diagnostics::error_in_context(
                "retained Terminal product",
                "source-evaluated import realization requires one native proposal",
            )
        })?;
        self.image_request
            .settle_application_metadata(
                proposal.native_target(),
                proposal.application().intent,
                proposal.application().subsystem,
                proposal.application().identifier.as_ref(),
            )
            .map_err(|error| {
                use target::ImageApplicationMetadataError;
                diagnostics::error_in_context(
                    "Terminal native proposal",
                    match error {
                        ImageApplicationMetadataError::SubsystemMismatch =>
                            "image request subsystem differs from the retained build subsystem",
                        ImageApplicationMetadataError::IdentifierMismatch =>
                            "image request code signature identifier differs from the retained build identifier",
                        ImageApplicationMetadataError::MissingGuiIdentifier =>
                            "signed macOS GUI image emission requires the retained build identifier",
                    },
                )
            })?;
        Ok((retained.shared_artifact(), proposal))
    }
}
