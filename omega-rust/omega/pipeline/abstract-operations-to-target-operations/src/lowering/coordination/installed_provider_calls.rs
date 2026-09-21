//! Installed provider-call evidence admission beside the lowering
//! coordinator: index the installation's admitted calls and the plan's
//! boundary calls, then rejoin each installed call's recorded evidence
//! against the exact boundary operation and its declaration.

use super::super::shared::*;

/// Identity of one boundary call occurrence: the caller machine, the Terminal
/// operation, and the boundary machine it enters.
pub(super) type BoundaryCallKey = (MachineId, OperationId, BoundaryMachineId);

/// Installed provider calls keyed by the boundary call occurrence they claim.
pub(super) type InstalledCallsByCall = BTreeMap<BoundaryCallKey, InstalledProviderCallEvidence>;

/// The plan's boundary call operations keyed by the same occurrence identity.
pub(super) type BoundaryCallsByKey<'a> = BTreeMap<BoundaryCallKey, &'a AbstractOperation>;

pub(super) fn index_installed_provider_calls(
    plan: &AbstractOperationPlan,
    installation: Option<&dyn ProviderInstallationEvidence>,
) -> Result<InstalledCallsByCall, LoweringError> {
    let installed_calls = installation
        .map(|installation| {
            if installation.psi() != plan.psi {
                return Err(LoweringError::ProviderInstallationIdentityMismatch);
            }
            Ok(installation.installed_provider_calls())
        })
        .transpose()?
        .unwrap_or_default();
    let mut installed_by_call = InstalledCallsByCall::new();
    for installed in installed_calls {
        let key = (
            installed.caller,
            installed.psi_operation,
            installed.boundary,
        );
        if installed_by_call.insert(key, installed).is_some() {
            return Err(LoweringError::DuplicateInstalledProviderCall {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
    Ok(installed_by_call)
}

pub(super) fn index_boundary_calls(plan: &AbstractOperationPlan) -> BoundaryCallsByKey<'_> {
    plan.functions
        .iter()
        .flat_map(|function| {
            function
                .operations
                .iter()
                .filter_map(move |operation| match operation {
                    AbstractOperation::BoundaryCall {
                        psi_operation,
                        boundary,
                        ..
                    } => Some(((function.machine, *psi_operation, *boundary), operation)),
                    _ => None,
                })
        })
        .collect()
}

pub(super) fn validate_installed_provider_calls(
    plan: &AbstractOperationPlan,
    installed_by_call: &InstalledCallsByCall,
    boundary_calls: &BoundaryCallsByKey<'_>,
) -> Result<(), LoweringError> {
    for (key, installed) in installed_by_call {
        let Some(AbstractOperation::BoundaryCall {
            result,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        }) = boundary_calls.get(key).copied()
        else {
            return Err(LoweringError::UnknownInstalledProviderCall {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        };
        let exact_sources = completion_claim_sources
            .iter()
            .map(|source| InstalledProviderCompletionClaimSource {
                claim: source.claim,
                entry: source.entry.clone(),
                content: source.content.clone(),
            })
            .collect::<Vec<_>>();
        let exact_result = match (result, &installed.result) {
            (
                abstract_operations::AbstractBoundaryResult::Unit,
                terminal_psi::OperationResult::Unit,
            ) => true,
            (
                abstract_operations::AbstractBoundaryResult::Structural(actual),
                terminal_psi::OperationResult::Structural(expected),
            ) => actual == expected,
            _ => false,
        };
        let declared_result_matches = plan
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == key.2)
            .is_some_and(|declaration| match (result, &declaration.result) {
                (
                    abstract_operations::AbstractBoundaryResult::Unit,
                    terminal_psi::BoundaryMachineResult::Unit,
                ) => true,
                (
                    abstract_operations::AbstractBoundaryResult::Structural(actual),
                    terminal_psi::BoundaryMachineResult::Structural(expected),
                ) => {
                    actual.structural_type == expected.structural_type
                        && actual.multiplicity == expected.multiplicity
                        && actual.qualifications == expected.qualifications
                }
                _ => false,
            });
        if !exact_result
            || !declared_result_matches
            || installed.scalar_arguments != *arguments
            || installed.structural_arguments != *structural_arguments
            || installed.completion_claim_sources != exact_sources
            || installed.completion_receipts != *completion_receipts
            || installed.provider.boundary != key.2
            || !plan
                .provider_candidates
                .iter()
                .any(|candidate| candidate == &installed.provider)
        {
            return Err(LoweringError::InstalledProviderCallEvidenceMismatch {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
    Ok(())
}
