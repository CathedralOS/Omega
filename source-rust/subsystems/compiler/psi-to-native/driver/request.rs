use verified_optimization_input::TerminalNativeRealizationProposal;
use diagnostics::Diagnostic;
use optimization_core::OptimizationWorkBudget;
use provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};
use target::ExecutableImageEmissionRequest;

/// Per-pass traversal and validation ceilings for the reference extent (2^16 units).
pub(crate) fn native_optimization_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 100_000, 4_000_000, 100_000, 1_000_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}

pub(crate) struct NativeCompileRequest<'request> {
    pub(crate) suppliers: &'request super::inputs::components::VerifiedSuppliers,
    /// Retain original stage inputs for explicitly requested correspondence audits.
    pub retain_native_stage_inputs: bool,
    pub private_entry_ram: &'request [psi_entry_memory::PrivateEntryRamInitialization],
    pub installation_assumptions:
        &'request [installation_conditions::InstallationEnvironmentAssumption],
    pub isolated_execution_grant:
        Option<&'request semantic_vocabulary::TerminalAuthorityDisposition>,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub image_request: ExecutableImageEmissionRequest,
    pub scalar_receipts: &'request mut Vec<native_scalar_inputs::EvaluationReceipt>,
}

pub(crate) fn image_request(
    proposal: &TerminalNativeRealizationProposal,
) -> Result<ExecutableImageEmissionRequest, Vec<Diagnostic>> {
    let application = proposal.application();
    let mut image = ExecutableImageEmissionRequest::direct(application.subsystem);
    image
        .settle_application_metadata(
            proposal.native_target(),
            application.intent,
            application.subsystem,
            application.identifier.as_ref(),
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
    Ok(image)
}
