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
    let view = artifact.view();
    // Historical compatibility is required even when folding erased every
    // executable boundary and the ordinary installation would be absent.
    let dependencies = backend_plan::matching_folded_constant_dependencies(
        view,
        request.profile.target_name(),
        request.selected_provider_plans.plans().iter(),
    )
    .map_err(|reason| realization_error("folded constant realization closure", reason))?;
    if plan.provider_candidates.is_empty() || request.checked_adapters.is_empty() {
        if view.module().folded_constants.target.is_none() {
            return Ok(None);
        }
        let selection = terminal_installation::admit_provider_installation(view, &[], &[])
            .map_err(|error| {
                realization_error("historical-only catalog admission", format!("{error:?}"))
            })?
            .admit_folded_constant_closure(view, request.profile.target_name(), &dependencies)
            .map_err(|reason| realization_error("folded constant realization closure", reason))?;
        return Ok(Some(AdmittedProviderInstallation {
            psi: selection.terminal_psi(),
            terminal_selection: selection,
            installed_candidates: Vec::new(),
            installed_calls: Vec::new(),
        }));
    }
    let mut installation =
        crate::provider_installation::admit_provider_installation_with_projection(
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
        .map_err(|error| {
            realization_error("checked-provider installation", format!("{error:?}"))
        })?;
    if view.module().folded_constants.target.is_some() {
        installation.terminal_selection = installation
            .terminal_selection
            .admit_folded_constant_closure(view, request.profile.target_name(), &dependencies)
            .map_err(|reason| realization_error("folded constant realization closure", reason))?;
    }
    Ok(Some(installation))
}
