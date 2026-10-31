//! Selected internal callees share ordinary ABI transport, not selection authority.
//! Provider installation and portable family selection retain distinct origins.

use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
use installation_evidence::InstalledProviderCallEvidence;
use semantic_vocabulary::{BoundaryMachineId, InvocationSite, MachineId};
use std::collections::BTreeMap;
use target_operations::NativeCallOrigin;
use terminal_psi::{TargetMachineCallable, TargetMachineSupply};

pub(crate) struct SelectedBoundaryCall {
    pub caller: MachineId,
    pub invocation_site: InvocationSite,
    pub boundary: BoundaryMachineId,
    pub callee: MachineId,
    pub result: AbstractBoundaryResult,
    pub origin: NativeCallOrigin,
}

pub(crate) type SelectedCallsByCall =
    BTreeMap<(MachineId, InvocationSite, BoundaryMachineId), SelectedBoundaryCall>;

pub(crate) fn index(
    plan: &AbstractOperationPlan,
    installed: BTreeMap<
        (MachineId, InvocationSite, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<SelectedCallsByCall, LoweringError> {
    let mut selected = SelectedCallsByCall::new();
    for function in &plan.functions {
        for operation in &function.operations {
            let (invocation_site, boundary, result, completion_claim_sources, completion_receipts) =
                match operation {
                    AbstractOperation::BoundaryCall {
                        psi_operation,
                        boundary,
                        result,
                        completion_claim_sources,
                        completion_receipts,
                        ..
                    } => (
                        (*psi_operation).into(),
                        boundary,
                        result,
                        completion_claim_sources.as_slice(),
                        completion_receipts.as_slice(),
                    ),
                    AbstractOperation::ExternalCompletion {
                        psi_edge, boundary, ..
                    } => (
                        InvocationSite::ExternalCompletion(*psi_edge),
                        boundary,
                        &AbstractBoundaryResult::Unit,
                        &[][..],
                        &[][..],
                    ),
                    _ => continue,
                };
            let key = (function.machine, invocation_site, *boundary);
            let invalid = || LoweringError::SelectedBoundaryCallEvidenceMismatch {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            };
            let installed_call = installed.get(&key);
            let family = input.and_then(|input| {
                input
                    .context()
                    .module()
                    .target_machine_families
                    .iter()
                    .find(|family| {
                        family.canonical == TargetMachineCallable::Boundary(*boundary)
                            && !family
                                .members
                                .iter()
                                .any(|member| member.callable == family.canonical)
                    })
            });
            let entry_call = input
                .and_then(|input| input.selected_program_entry_call())
                .filter(|entry| entry.boundary() == *boundary);
            let (callee, origin) = if let Some(entry) = entry_call {
                if installed_call.is_some()
                    || family.is_some()
                    || plan.function(entry.entry()).is_none()
                {
                    return Err(invalid());
                }
                (
                    entry.entry(),
                    NativeCallOrigin::ProgramEntry {
                        boundary: *boundary,
                        entry: entry.entry(),
                        source_signature: entry.source_signature(),
                        completion_claim_sources: completion_claim_sources.to_vec(),
                        completion_receipts: completion_receipts.to_vec(),
                    },
                )
            } else {
                match (installed_call, family) {
                    (Some(_), Some(_)) => return Err(invalid()),
                    (Some(call), None) => (
                        call.provider.candidate,
                        NativeCallOrigin::InstalledProvider {
                            boundary: *boundary,
                            provider: call.provider.clone(),
                            assumption: call.assumption.clone(),
                            completion_claim_sources: completion_claim_sources.to_vec(),
                            completion_receipts: completion_receipts.to_vec(),
                        },
                    ),
                    (None, Some(family)) => {
                        let input = input.ok_or_else(invalid)?;
                        let selections = input.target_machine_selections().ok_or_else(invalid)?;
                        let member = selections.member(family.canonical).map_err(|_| invalid())?;
                        let TargetMachineCallable::Machine(callee) = member.callable() else {
                            // Opaque selected members use ordinary provider settlement
                            // under the unchanged head occurrence and contract.
                            continue;
                        };
                        if selections.terminal_psi() != plan.psi
                            || !matches!(
                                member.supply(),
                                TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary
                            )
                            || family
                                .members
                                .iter()
                                .filter(|row| {
                                    row.target == selections.target()
                                        && row.callable == member.callable()
                                        && row.supply == member.supply()
                                })
                                .count()
                                != 1
                            || plan.function(callee).is_none()
                        {
                            return Err(invalid());
                        }
                        (
                            callee,
                            NativeCallOrigin::TargetFamily {
                                boundary: *boundary,
                                member: callee,
                                target: selections.target().to_owned(),
                                completion_claim_sources: completion_claim_sources.to_vec(),
                                completion_receipts: completion_receipts.to_vec(),
                            },
                        )
                    }
                    (None, None) => continue,
                }
            };
            if selected
                .insert(
                    key,
                    SelectedBoundaryCall {
                        caller: key.0,
                        invocation_site: key.1,
                        boundary: key.2,
                        callee,
                        result: result.clone(),
                        origin,
                    },
                )
                .is_some()
            {
                return Err(LoweringError::DuplicateSelectedBoundaryCall {
                    machine: key.0,
                    operation: key.1,
                    boundary: key.2,
                });
            }
        }
    }
    Ok(selected)
}
