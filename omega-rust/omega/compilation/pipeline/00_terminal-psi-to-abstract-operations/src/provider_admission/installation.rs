use crate::AdmittedProviderInstallation;
use crate::provider_admission::diagnostics::realization_error;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use diagnostics::Diagnostic;

use super::adapters::project_selected_provider_adapters;

pub(crate) fn admit_checked_provider_installation(
    input: &NativeRealizationInput,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
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
    let view = artifact.view();
    let installation = crate::provider_installation::admit_provider_installation_with_projection(
        plan, view, &selected,
    )
    .map_err(|error| realization_error("checked-provider installation", format!("{error:?}")))?;
    Ok(Some(installation))
}
