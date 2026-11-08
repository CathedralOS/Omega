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

mod replay;

use abstract_operations::{AbstractOperationPlan, CompletionClaimSource};
use installation_evidence::InstalledProviderCallEvidence;
use replay::replay_installed_provider_calls;
use semantic_vocabulary::{InvocationSite, MachineId, OperationId};
use terminal_psi::{CompletionReceipt, ProviderCandidateConformance, StructuralArgument};

#[derive(Debug)]
pub enum ProviderInstallationError {
    PlanReplayMismatch,
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
    PsiAdmission(terminal_installation::ProviderInstallationError),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderAdapter {
    pub provider_declaration: String, // Selecting declaration, independent of the realizing callable.
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest, // Full originating plan identity; report fingerprints cannot select it.
    pub selected_requirement_identity: String, // Original row before specialization or target-family aliasing.
    pub requirement_identity: String,
    pub provider_identity: String,
    pub machine_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledProviderCall {
    pub(crate) caller: MachineId,
    pub(crate) invocation_site: InvocationSite,
    pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
    pub(crate) provider: ProviderCandidateConformance, // Supplied requirement's real checked conformance, never relabelled.
    pub(crate) assumption: Option<terminal_installation::AdmittedAssumedBinding>,
    pub(crate) result: terminal_psi::OperationResult,
    pub(crate) scalar_arguments: Vec<semantic_vocabulary::ValueId>,
    pub(crate) structural_arguments: Vec<StructuralArgument>,
    pub(crate) completion_claim_sources: Vec<CompletionClaimSource>,
    pub(crate) completion_receipts: Vec<CompletionReceipt>,
}

impl AdmittedInstalledProviderCall {
    pub const fn caller(&self) -> MachineId {
        self.caller
    }

    pub const fn invocation_site(&self) -> InvocationSite {
        self.invocation_site
    }

    pub const fn boundary(&self) -> semantic_vocabulary::BoundaryMachineId {
        self.boundary
    }

    pub const fn provider(&self) -> &ProviderCandidateConformance {
        &self.provider
    }

    pub const fn result(&self) -> &terminal_psi::OperationResult {
        &self.result
    }

    pub fn scalar_arguments(&self) -> &[semantic_vocabulary::ValueId] {
        &self.scalar_arguments
    }

    pub fn structural_arguments(&self) -> &[StructuralArgument] {
        &self.structural_arguments
    }

    pub fn completion_claim_sources(&self) -> &[CompletionClaimSource] {
        &self.completion_claim_sources
    }

    pub fn completion_receipts(&self) -> &[CompletionReceipt] {
        &self.completion_receipts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderInstallation {
    pub(crate) terminal_selection: terminal_installation::AdmittedProviderSelections,
    pub(crate) psi: terminal_psi::TerminalPsiIdentity,
    pub(crate) installed_candidates: Vec<ProviderCandidateConformance>,
    pub(crate) installed_calls: Vec<AdmittedInstalledProviderCall>,
}

impl AdmittedProviderInstallation {
    pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    pub const fn terminal_selection(&self) -> &terminal_installation::AdmittedProviderSelections {
        &self.terminal_selection
    }

    pub(crate) fn installed_candidates(&self) -> &[ProviderCandidateConformance] {
        &self.installed_candidates
    }

    pub fn installed_calls(&self) -> &[AdmittedInstalledProviderCall] {
        &self.installed_calls
    }
}

impl installation_evidence::ProviderInstallationEvidence for AdmittedProviderInstallation {
    fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.psi
    }

    fn installed_provider_calls(&self) -> Vec<InstalledProviderCallEvidence> {
        self.installed_calls
            .iter()
            .map(|call| InstalledProviderCallEvidence {
                caller: call.caller,
                invocation_site: call.invocation_site,
                boundary: call.boundary,
                provider: call.provider.clone(),
                assumption: call.assumption.as_ref().map(|row| {
                    installation_evidence::InstalledAssumedBindingEvidence {
                        required: row.required,
                        supplied_requirement: row.supplied_requirement,
                        implementation: row.implementation,
                        selected_requirement: row.selected_requirement,
                        supply_path: row.supply_path.clone(),
                    }
                }),
                result: call.result.clone(),
                scalar_arguments: call.scalar_arguments.clone(),
                structural_arguments: call.structural_arguments.clone(),
                completion_claim_sources: call
                    .completion_claim_sources
                    .iter()
                    .map(
                        |source| installation_evidence::InstalledProviderCompletionClaimSource {
                            claim: source.claim,
                            entry: source.entry.clone(),
                            content: source.content.clone(),
                        },
                    )
                    .collect(),
                completion_receipts: call.completion_receipts.clone(),
            })
            .collect()
    }
}

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
