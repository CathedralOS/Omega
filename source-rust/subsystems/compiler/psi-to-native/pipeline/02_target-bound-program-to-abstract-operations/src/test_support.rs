//! Direct lowering of controlled fixtures without a published deployment selection.
use super::admit_providers::artifact_lowering_error;
use super::{
    AdmittedNativeRealization, ArtifactLoweringError, NativeAdmissionRequest,
    VerifiedNativeArtifactInput, admit_prepared_native_realization,
};
pub use crate::admit_providers::prepare_native_providers;
pub use crate::selected_execution::lower_native_execution;
pub use crate::selected_input::NativeExecutionSelection;
use diagnostics::Diagnostic;
use provider_admission::realization_error;
use verified_optimization_input::VerifiedPsiOptimizationInput;

/// Lower the authorized fixture's default entry and attach its optional evidence seals.
pub fn lower_artifact(
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    evidence: Option<terminal_codec::EvidenceSeals>,
) -> Result<VerifiedPsiOptimizationInput, ArtifactLoweringError> {
    let module = authorization.artifact().semantic_module();
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        return Err(ArtifactLoweringError::TargetSelectionRequired);
    };
    let plan = super::operations::lower_selected_module(
        module,
        entry,
        module.types.structural_types.clone(),
        None,
        None,
        None,
    )
    .map_err(ArtifactLoweringError::Lowering)?;
    let mut input = VerifiedPsiOptimizationInput::lowered(plan, authorization.clone(), None)
        .map_err(|reason| ArtifactLoweringError::TargetContext(reason.to_owned()))?;
    if let Some(evidence) = evidence {
        if input.plan().psi != evidence.module {
            return Err(ArtifactLoweringError::EvidenceForAnotherModule);
        }
        input.retain_evidence_seals(evidence.seals);
    }
    Ok(input)
}

pub fn admit_native_realization<'execution>(
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    request: &NativeAdmissionRequest<'_, 'execution>,
) -> Result<AdmittedNativeRealization<'execution>, Vec<Diagnostic>> {
    let input = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
        authorization,
        request.providers.profile,
        request.entry,
        request.private_entry_ram,
        &NativeExecutionSelection {
            scope: None,
            scalar_applications: None,
            providers: request.providers.checked_adapters,
            plans: request.providers.selected_provider_plans,
            callbacks: &[],
            entry_call: None,
        },
    )
    .map_err(artifact_lowering_error)?;
    let artifact = authorization.artifact();
    let input = input
        .bind_selected_provider_coordinates(artifact, request.providers.selected_provider_plans)
        .map_err(|error| realization_error("selected provider coordinates", error))?;
    admit_prepared_native_realization(input, artifact, request)
}
