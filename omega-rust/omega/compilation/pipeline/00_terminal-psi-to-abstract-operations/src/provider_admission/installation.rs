use crate::AdmittedProviderInstallation;
use crate::provider_admission::diagnostics::realization_error;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use diagnostics::Diagnostic;

pub(crate) fn admit_checked_provider_installation(
    input: &NativeRealizationInput,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
) -> Result<Option<AdmittedProviderInstallation>, Vec<Diagnostic>> {
    let plan = input.plan();
    if plan.provider_candidates.is_empty() {
        return Ok(None);
    }
    if request.checked_adapters.is_empty() {
        return Ok(None);
    }
    let view = artifact.view();
    let installation = crate::provider_installation::admit_provider_installation_with_projection(
        plan,
        view,
        request.checked_adapters,
        &request
            .selected_provider_plans
            .assumed_binding_coordinates(view.module())
            .map_err(|reason| realization_error("assumed binding installation", reason))?
            .into_iter()
            .map(|(required, supplied_requirement)| {
                terminal_installation::AssumedBindingSelection {
                    required,
                    supplied_requirement,
                }
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|error| realization_error("checked-provider installation", format!("{error:?}")))?;
    Ok(Some(installation))
}
