use super::error::ProviderInstallationError;
use super::replay::replay_installed_provider_calls;
use super::{AdmittedProviderInstallation, SelectedProviderAdapter};
use abstract_operations::AbstractOperationPlan;

/// Admit provider policy against the same immutable artifact used to lower
/// the plan. The view binds the module to its canonical identity; external
/// ingress owns decoding, not each consumer of a retained compiler product.
pub(crate) fn admit_provider_installation_with_projection(
    plan: &AbstractOperationPlan,
    artifact: terminal_codec::TerminalArtifactView<'_>,
    selected: &[SelectedProviderAdapter],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    if artifact.identity() != plan.psi {
        return Err(ProviderInstallationError::TerminalIdentityMismatch);
    }
    let module = artifact.module();
    let mut selections = Vec::new();
    let mut installed_candidates = Vec::new();
    let mut boundaries = plan
        .provider_candidates
        .iter()
        .map(|candidate| candidate.boundary)
        .collect::<Vec<_>>();
    boundaries.sort();
    boundaries.dedup();
    for boundary in boundaries {
        let candidates = plan
            .provider_candidates
            .iter()
            .filter(|candidate| candidate.boundary == boundary)
            .collect::<Vec<_>>();
        let requirement_identity = candidates[0].requirement_identity.as_str();
        if requirement_identity.is_empty()
            || candidates
                .iter()
                .any(|candidate| candidate.requirement_identity != requirement_identity)
        {
            return Err(ProviderInstallationError::InvalidLoweredCatalog);
        }
        let selected_rows = selected
            .iter()
            .filter(|row| row.requirement_identity == requirement_identity)
            .map(|row| {
                (
                    row.provider_identity.as_str(),
                    row.machine_identity.as_str(),
                )
            })
            .collect::<Vec<_>>();
        if selected_rows.is_empty() {
            return Err(ProviderInstallationError::MissingSelectedProvider { boundary });
        }
        let exact = candidates
            .iter()
            .filter(|candidate| {
                selected_rows.iter().any(|(provider, machine)| {
                    candidate.provider_identity == *provider
                        && candidate.candidate_identity == *machine
                })
            })
            .collect::<Vec<_>>();
        let [candidate] = exact.as_slice() else {
            return Err(if exact.is_empty() {
                ProviderInstallationError::SelectedProviderMismatch { boundary }
            } else {
                ProviderInstallationError::AmbiguousSelectedProvider { boundary }
            });
        };
        selections.push(terminal_installation::ProviderInstallationSelection {
            boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        });
        installed_candidates.push((**candidate).clone());
    }
    let installed_calls = replay_installed_provider_calls(plan, module, &installed_candidates)?;
    let installation =
        terminal_installation::admit_provider_installation(artifact, &selections, &[])
            .map_err(ProviderInstallationError::PsiAdmission)?;
    Ok(AdmittedProviderInstallation {
        psi: installation.terminal_psi(),
        terminal_selection: installation,
        installed_candidates,
        installed_calls,
    })
}
