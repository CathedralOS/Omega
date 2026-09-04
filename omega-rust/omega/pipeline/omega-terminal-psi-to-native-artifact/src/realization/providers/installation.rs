use crate::realization::diagnostics::realization_error;
use crate::realization::model::{
    NativeRealizationCoreRequest, NativeRealizationInput, PostTerminalOptimizationContinuation,
};
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

use super::adapters::project_selected_provider_adapters;

pub(crate) fn admit_checked_provider_installation(
    input: &NativeRealizationInput,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<Option<AdmittedProviderInstallation>, Vec<Diagnostic>> {
    let plan = input.plan();
    if plan.provider_candidates.is_empty() {
        return Ok(None);
    }
    let selected = project_selected_provider_adapters(request.selected_provider_plans, plan)
        .map_err(|error| realization_error("selected checked-provider projection", error))?;
    if selected.is_empty() {
        return Ok(None);
    }
    let installation = match input.optimization_continuation() {
        PostTerminalOptimizationContinuation::Selected(_) => {
            omega_psi_to_abstract_operations::admit_provider_installation_for_optimization(
                plan,
                semantic_bytes,
                proof_bytes,
                request.profile,
                &selected,
            )
        }
        PostTerminalOptimizationContinuation::Identity(_) => match input.native() {
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(_) => {
                omega_psi_to_abstract_operations::admit_provider_installation(
                    plan,
                    semantic_bytes,
                    proof_bytes,
                    request.profile,
                    &selected,
                )
            }
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
                _,
            ) => {
                return Ok(None);
            }
        },
    }
    .map_err(|error| realization_error("checked-provider installation", format!("{error:?}")))?;
    Ok(Some(installation))
}
