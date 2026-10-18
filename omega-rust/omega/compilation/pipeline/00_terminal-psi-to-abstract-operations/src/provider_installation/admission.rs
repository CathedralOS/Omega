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
    assumptions: &[terminal_installation::AssumedBindingSelection],
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
        let requirement_identity = plan.identities.text(candidates[0].requirement_identity);
        if requirement_identity.unwrap_or_default().is_empty()
            || candidates.iter().any(|candidate| {
                plan.identities.text(candidate.requirement_identity) != requirement_identity
            })
        {
            return Err(ProviderInstallationError::InvalidLoweredCatalog);
        }
        let selected_rows = selected
            .iter()
            .filter(|row| row.requirement_identity == requirement_identity.unwrap_or_default())
            .map(|row| {
                (
                    row.provider_declaration.as_str(),
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
                selected_rows
                    .iter()
                    .any(|(declaration, provider, machine)| {
                        plan.identities
                            .text(candidate.provider_declaration.identity())
                            == Some(*declaration)
                            && plan.identities.text(candidate.provider_identity) == Some(*provider)
                            && plan.identities.text(candidate.candidate_identity) == Some(*machine)
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
            provider_identity: plan
                .identities
                .text(candidate.provider_identity)
                .unwrap_or_default()
                .to_string(),
            candidate: candidate.candidate,
            provider_declaration: plan
                .identities
                .text(candidate.provider_declaration.identity())
                .ok_or(ProviderInstallationError::InvalidLoweredCatalog)?
                .to_owned(),
        });
        installed_candidates.push((**candidate).clone());
    }
    let installation =
        terminal_installation::admit_provider_installation(artifact, &selections, assumptions)
            .map_err(ProviderInstallationError::PsiAdmission)?;
    let installed_calls = replay_installed_provider_calls(
        plan,
        module,
        &installed_candidates,
        installation.assumed_bindings(),
    )?;
    Ok(AdmittedProviderInstallation {
        psi: installation.terminal_psi(),
        terminal_selection: installation,
        installed_candidates,
        installed_calls,
    })
}
