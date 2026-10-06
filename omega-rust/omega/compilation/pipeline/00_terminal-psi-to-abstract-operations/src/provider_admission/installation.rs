use crate::AdmittedProviderInstallation;
use crate::provider_admission::diagnostics::realization_error;
use crate::provider_admission::{NativeRealizationInput, ProviderAdmissionRequest};
use diagnostics::Diagnostic;

use super::adapters::project_selected_provider_adapters;

pub fn admit_checked_provider_installation(
    input: &NativeRealizationInput,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_>,
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
    // The artifact replayed its sections before admission reached here, so
    // its canonical module is already decoded; installation reads that
    // retained decoding rather than running the same codec validations over
    // the same bytes a second time.
    let module = artifact.semantic_module().map_err(|error| {
        realization_error("checked-provider installation", format!("{error:?}"))
    })?;
    let installation = crate::provider_installation::admit_provider_installation_with_projection(
        plan,
        module,
        artifact.semantic_bytes(),
        &selected,
    )
    .map_err(|error| realization_error("checked-provider installation", format!("{error:?}")))?;
    Ok(Some(installation))
}
