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
    let (selections, installed_candidates) =
        select_provider_candidates(&plan.provider_candidates, &plan.identities, selected)?;
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

/// Admit checked bodies explicitly selected by the receiving executor.
/// Unselected catalog rows remain unavailable. Native realization uses the
/// same matcher; Terminal admission independently checks its selected rows.
pub fn admit_terminal_provider_installation(
    artifact: terminal_codec::TerminalExecutionView<'_>,
    selected: &[SelectedProviderAdapter],
) -> Result<terminal_installation::AdmittedProviderSelections, ProviderInstallationError> {
    let module = artifact.module();
    let candidates = module
        .provider_candidates
        .iter()
        .filter(|candidate| {
            selected.iter().any(|row| {
                module.identities.text(candidate.requirement_identity)
                    == Some(row.requirement_identity.as_str())
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let (selections, _) = select_provider_candidates(&candidates, &module.identities, selected)?;
    terminal_installation::admit_provider_execution_installation(artifact, &selections, &[])
        .map_err(ProviderInstallationError::PsiAdmission)
}

fn select_provider_candidates(
    candidates: &[terminal_psi::ProviderCandidateConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    selected: &[SelectedProviderAdapter],
) -> Result<
    (
        Vec<terminal_installation::ProviderInstallationSelection>,
        Vec<terminal_psi::ProviderCandidateConformance>,
    ),
    ProviderInstallationError,
> {
    let mut selections = Vec::new();
    let mut installed_candidates = Vec::new();
    let mut candidates_by_boundary = std::collections::BTreeMap::<
        semantic_vocabulary::BoundaryMachineId,
        Vec<&terminal_psi::ProviderCandidateConformance>,
    >::new();
    for candidate in candidates {
        candidates_by_boundary
            .entry(candidate.boundary)
            .or_default()
            .push(candidate);
    }
    let mut selected_by_req = std::collections::BTreeMap::<&str, Vec<(&str, &str, &str)>>::new();
    for row in selected {
        selected_by_req
            .entry(row.requirement_identity.as_str())
            .or_default()
            .push((
                row.provider_declaration.as_str(),
                row.provider_identity.as_str(),
                row.machine_identity.as_str(),
            ));
    }
    for (boundary, candidates) in candidates_by_boundary {
        let requirement_identity = identities.text(candidates[0].requirement_identity);
        if requirement_identity.unwrap_or_default().is_empty()
            || candidates.iter().any(|candidate| {
                identities.text(candidate.requirement_identity) != requirement_identity
            })
        {
            return Err(ProviderInstallationError::InvalidLoweredCatalog);
        }
        let Some(selected_rows) = selected_by_req.get(requirement_identity.unwrap_or_default())
        else {
            return Err(ProviderInstallationError::MissingSelectedProvider { boundary });
        };
        let exact = candidates
            .iter()
            .filter(|candidate| {
                selected_rows
                    .iter()
                    .any(|(declaration, provider, machine)| {
                        identities.text(candidate.provider_declaration.identity())
                            == Some(*declaration)
                            && identities.text(candidate.provider_identity) == Some(*provider)
                            && identities.text(candidate.candidate_identity) == Some(*machine)
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
            provider_identity: identities
                .text(candidate.provider_identity)
                .unwrap_or_default()
                .to_string(),
            candidate: candidate.candidate,
            provider_declaration: identities
                .text(candidate.provider_declaration.identity())
                .ok_or(ProviderInstallationError::InvalidLoweredCatalog)?
                .to_owned(),
        });
        installed_candidates.push((**candidate).clone());
    }
    Ok((selections, installed_candidates))
}
