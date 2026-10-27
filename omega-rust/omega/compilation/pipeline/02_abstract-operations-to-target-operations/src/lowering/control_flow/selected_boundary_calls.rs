//! Selected boundary calls use ordinary call transport, retaining their origin.
//!
//! Coordination has joined each selected occurrence to the exact independently
//! admitted Terminal artifact. Selection changes the callee, not the caller's
//! operands, result place or completion receipts. This temporary call projection
//! is only an input to shared ABI/storage lowering; it never replaces the
//! original BoundaryCall in the abstract plan. The target origin keeps that
//! distinction available to independent legalization and physical replay.

use crate::LoweringError;
use crate::lowering::selected_boundary_calls::SelectedBoundaryCall;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
};
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;
use target_operations::TargetUnitOperation;

pub(super) fn resolve(
    operation: &AbstractOperation,
    installed: &SelectedBoundaryCall,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Result<(AbstractOperation, target_operations::NativeCallOrigin), LoweringError> {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources: _,
        completion_receipts,
    } = operation
    else {
        return Err(LoweringError::SelectedBoundaryCallEvidenceMismatch {
            machine: installed.caller,
            operation: installed.invocation_site,
            boundary: installed.boundary,
        });
    };
    let invalid = || LoweringError::SelectedBoundaryCallShapeMismatch {
        machine: installed.caller,
        operation: *psi_operation,
        boundary: *boundary,
    };
    if result != &installed.result {
        return Err(invalid());
    }
    let callee = installed.callee;
    let candidate = functions.get(&callee).copied().ok_or_else(invalid)?;
    let claim_transfers = completion_receipts
        .iter()
        .map(|receipt| terminal_psi::ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
            path: receipt.path.clone(),
        })
        .collect::<Vec<_>>();
    let resolved = match (result, &candidate.result) {
        (abstract_operations::AbstractBoundaryResult::Unit, AbstractFunctionResult::Unit) => {
            AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                callee: callee.into(),
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                result: AbstractStructuralCallResult::Unit,
            }
        }
        (
            abstract_operations::AbstractBoundaryResult::Structural(result),
            AbstractFunctionResult::Structural(candidate_result),
        ) if result.structural_type == candidate_result.structural_type
            && result.multiplicity == candidate_result.multiplicity =>
        {
            // The caller-visible occurrence joins the exact admitted
            // caller-local result, not the candidate's declared roster.
            // Provider conformance or the checked whole-family interface
            // owns the semantic refinement. This shared projection transports
            // the same carrier and multiplicity; it creates no result domain,
            // private guarantee, provider grant or completion authority.
            AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                result: AbstractStructuralCallResult::Structural {
                    result: result.clone(),
                    selected_evidence: Vec::new(),
                },
                callee: callee.into(),
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }
        // A selected scalar callee is an ordinary scalar-result call. Without
        // structural operands it is the scalar-only `Call`, whose callee must
        // publish a standalone scalar ABI; otherwise a scalar-result
        // `CallStructural` carries its borrowed operands. Both lower through
        // `calls`.
        (
            abstract_operations::AbstractBoundaryResult::Scalar(result),
            AbstractFunctionResult::Scalar(candidate_result),
        ) if result.scalar_type == candidate_result.scalar_type => {
            if structural_arguments.is_empty() && claim_transfers.is_empty() {
                AbstractOperation::Call {
                    psi_operation: *psi_operation,
                    result: result.value,
                    scalar_type: result.scalar_type,
                    callee,
                    arguments: arguments.clone(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            } else {
                AbstractOperation::CallStructural {
                    psi_operation: *psi_operation,
                    result: AbstractStructuralCallResult::Scalar(*result),
                    callee: callee.into(),
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            }
        }
        _ => return Err(invalid()),
    };
    let origin = installed.origin.clone();
    Ok((resolved, origin))
}

pub(super) fn retain_origin(
    operations: &mut [TargetUnitOperation],
    installed: &SelectedBoundaryCall,
    origin: target_operations::NativeCallOrigin,
) -> Result<(), LoweringError> {
    let [
        TargetUnitOperation::Call {
            psi_operation,
            target,
            origin: actual,
            ..
        },
    ] = operations
    else {
        return Err(LoweringError::SelectedBoundaryCallShapeMismatch {
            machine: installed.caller,
            operation: installed.invocation_site.operation().ok_or(
                LoweringError::BoundaryRealizationMismatch(installed.boundary),
            )?,
            boundary: installed.boundary,
        });
    };
    if semantic_vocabulary::InvocationSite::Operation(*psi_operation) != installed.invocation_site
        || *target != target_operations::NativeCallTarget::Machine(installed.callee)
        || *actual != target_operations::NativeCallOrigin::Authored
    {
        return Err(LoweringError::SelectedBoundaryCallEvidenceMismatch {
            machine: installed.caller,
            operation: installed.invocation_site,
            boundary: installed.boundary,
        });
    }
    *actual = origin;
    Ok(())
}
