/*
Provider installation custody joining canonical candidates and boundary call
occurrences inside the invocation's selected abstract scope.

Canonical Terminal keeps the complete candidate catalog; scope selection
precedes installation so no later consumer has to discard orphaned call
receipts. Optimizations never select providers or redirect family heads.
*/

/*
We bind Omega's provider policy only to exact rows preserved from the verified
terminal catalog. Installation checks selections against the retained artifact
view before returning the private-field `AdmittedProviderInstallation` carrier
used by execution, preventing target lowering from reconstructing authority from
a candidate machine ID alone.
*/

#[path = "provider_installation_replay.rs"]
mod replay;

use abstract_operations::AbstractOperationPlan;
use operation_evidence::provider_calls::InstalledProviderCallEvidence;
use replay::replay_installed_provider_calls;
use semantic_vocabulary::{InvocationSite, MachineId, OperationId};
use terminal_psi::ProviderCandidateConformance;

#[derive(Debug)]
pub enum ProviderInstallationError {
    InvalidLoweredCatalog,
    MissingSelectedProvider {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    SelectedProviderMismatch {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    AmbiguousSelectedProvider {
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    PsiAdmission(psi_provider_selections::ProviderInstallationError),
    TerminalIdentityMismatch,
    InstalledContentClaimMismatch {
        caller: MachineId,
        operation: OperationId,
        boundary: semantic_vocabulary::BoundaryMachineId,
        claim: semantic_vocabulary::ClaimId,
        reason: &'static str,
    },
    InstalledCallReplayMismatch {
        caller: MachineId,
        invocation_site: InvocationSite,
        boundary: semantic_vocabulary::BoundaryMachineId,
    },
    ExternalCompletionResultMismatch {
        caller: MachineId,
        invocation_site: InvocationSite,
        boundary: semantic_vocabulary::BoundaryMachineId,
        reason: String,
    },
}

pub use target_bound_program::SelectedProviderAdapter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderInstallation {
    pub(crate) terminal_selection: psi_provider_selections::AdmittedProviderSelections,
    pub(crate) installed_candidates: Vec<ProviderCandidateConformance>,
    pub(crate) installed_calls: Vec<InstalledProviderCallEvidence>,
}

impl AdmittedProviderInstallation {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_selection.terminal_psi()
    }

    pub const fn terminal_selection(&self) -> &psi_provider_selections::AdmittedProviderSelections {
        &self.terminal_selection
    }

    pub(crate) fn installed_candidates(&self) -> &[ProviderCandidateConformance] {
        &self.installed_candidates
    }

    pub fn installed_provider_calls(&self) -> Vec<InstalledProviderCallEvidence> {
        self.installed_calls.clone()
    }
}

pub(crate) fn admit_provider_installation_with_projection(
    plan: &AbstractOperationPlan,
    artifact: terminal_codec::TerminalArtifactView<'_>,
    selected: &[SelectedProviderAdapter],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    if artifact.identity() != plan.psi {
        return Err(ProviderInstallationError::TerminalIdentityMismatch);
    }
    let module = artifact.module();
    let (selections, installed_candidates) =
        select_provider_candidates(&plan.provider_candidates, &plan.identities, selected)?;
    let installation =
        psi_provider_selections::admit_provider_installation(artifact, &selections)
            .map_err(ProviderInstallationError::PsiAdmission)?;
    let installed_calls = replay_installed_provider_calls(
        plan,
        module,
        &installed_candidates,
    )?;
    Ok(AdmittedProviderInstallation {
        terminal_selection: installation,
        installed_candidates,
        installed_calls,
    })
}

fn select_provider_candidates(
    candidates: &[terminal_psi::ProviderCandidateConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    selected: &[SelectedProviderAdapter],
) -> Result<
    (
        Vec<psi_provider_selections::ProviderInstallationSelection>,
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
        selections.push(psi_provider_selections::ProviderInstallationSelection {
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
