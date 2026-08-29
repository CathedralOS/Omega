use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

use super::adapters::project_selected_provider_adapters;

pub(crate) fn admit_checked_provider_installation(
    input: &NativeRealizationInput,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    request: &NativeRealizationRequest<'_>,
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
    let installation = match input {
        NativeRealizationInput::Ordinary(_) => {
            omega_psi_to_abstract_operations::admit_provider_installation(
                plan,
                semantic_bytes,
                proof_bytes,
                request.profile,
                &selected,
            )
        }
        NativeRealizationInput::ExplicitOptimization(_) => {
            omega_psi_to_abstract_operations::admit_provider_installation_for_optimization(
                plan,
                semantic_bytes,
                proof_bytes,
                request.profile,
                &selected,
            )
        }
    }
    .map_err(|error| realization_error("checked-provider installation", format!("{error:?}")))?;
    Ok(Some(installation))
}
