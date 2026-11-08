// Native compilation and realization requests (`NativeCompileRequest` and
// `NativeRealizationRequest`) and baseline optimizer work budgets.
// `native_optimization_budget` calibrates per-pass traversal and validation
// ceilings for one `OPTIMIZATION_REFERENCE_EXTENT` (2^16 units) so register
// pressure recovery, callee-save derivation, and fixed-occurrence linear audits
// scale with the function or program they inspect without changing candidate or
// commit limits.

use diagnostics::Diagnostic;
use image_emission::ExecutableImageEmissionRequest;
use optimization_core::OptimizationWorkBudget;
use terminal_psi_to_abstract_operations::provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};

pub(crate) struct NativeRealizationRequest<'request> {
    pub observation: &'request crate::observations::NativePipelineObservation,
    pub proposal: &'request terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal,
    pub execution: &'request terminal_psi_to_abstract_operations::PreparedNativeExecution,
    pub scalar_applications:
        Option<terminal_psi_to_abstract_operations::NativeScalarApplications<'request>>,
    pub program_entry: terminal_psi_to_abstract_operations::NativeProgramEntrySettlement<'request>,
    pub admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'request, 'request>,
    pub image_request: ExecutableImageEmissionRequest,
    pub elf_interpreter: Option<&'request target::NormalizedElfInterpreterPlan>,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub callback_thunks: &'request [backend_plan::NativeCallbackThunkSettlement<'request>],
    pub target_provider_proposals: &'request [backend_plan::TerminalTargetProviderProposal],
}

impl NativeRealizationRequest<'_> {
    pub fn optimization_budget(&self) -> OptimizationWorkBudget {
        native_optimization_budget()
    }
}

pub(crate) fn native_optimization_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 100_000, 4_000_000, 100_000, 1_000_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}

pub struct NativeCompileRequest<'request> {
    pub private_entry_ram: &'request [terminal_installation::PrivateEntryRamInitialization],
    pub installation_assumptions:
        &'request [installation_evidence::InstallationEnvironmentAssumption],
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub image_request: ExecutableImageEmissionRequest,
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
    pub components: &'request [crate::ComponentCompilation<'request>],
}

impl NativeCompileRequest<'_> {
    pub(crate) fn prepare_terminal<'input>(
        &mut self,
        retained: &'input terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    ) -> Result<
        (
            &'input std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
            &'input terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal,
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
                use image_emission::ImageApplicationMetadataError;
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
