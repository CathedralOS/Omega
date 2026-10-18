//! Selected internal callees share ordinary ABI transport, not selection authority.
//! Provider installation and portable family selection retain distinct origins.

use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AbstractOperationPlan};
use installation_evidence::InstalledProviderCallEvidence;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use std::collections::BTreeMap;
use target_operations::NativeCallOrigin;
use terminal_psi::{TargetMachineCallable, TargetMachineSupply};

pub(crate) struct SelectedBoundaryCall {
    pub caller: MachineId,
    pub psi_operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub callee: MachineId,
    pub result: AbstractBoundaryResult,
    pub origin: NativeCallOrigin,
}

pub(crate) type SelectedCallsByCall =
    BTreeMap<(MachineId, OperationId, BoundaryMachineId), SelectedBoundaryCall>;

pub(crate) fn index(
    plan: &AbstractOperationPlan,
    installed: BTreeMap<(MachineId, OperationId, BoundaryMachineId), InstalledProviderCallEvidence>,
    input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<SelectedCallsByCall, LoweringError> {
    let mut selected = SelectedCallsByCall::new();
    for function in &plan.functions {
        for operation in &function.operations {
            let AbstractOperation::BoundaryCall {
                psi_operation,
                boundary,
                result,
                completion_claim_sources,
                completion_receipts,
                ..
            } = operation
            else {
                continue;
            };
            let key = (function.machine, *psi_operation, *boundary);
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
            let (callee, origin) = match (installed_call, family) {
                (Some(_), Some(_)) => return Err(invalid()),
                (Some(call), None) => (
                    call.provider.candidate,
                    NativeCallOrigin::InstalledProvider {
                        boundary: *boundary,
                        provider: call.provider.clone(),
                        assumption: call.assumption.clone(),
                        completion_claim_sources: completion_claim_sources.clone(),
                        completion_receipts: completion_receipts.clone(),
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
                            completion_claim_sources: completion_claim_sources.clone(),
                            completion_receipts: completion_receipts.clone(),
                        },
                    )
                }
                (None, None) => continue,
            };
            if selected
                .insert(
                    key,
                    SelectedBoundaryCall {
                        caller: key.0,
                        psi_operation: key.1,
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
