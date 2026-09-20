//! Selected boundary calls use ordinary call transport, retaining their origin.
//!
//! Coordination has joined each installed occurrence to the exact independently
//! admitted Terminal artifact. Selection changes the callee, not the caller's
//! operands, result place or completion receipts. This temporary call projection
//! is only an input to shared ABI/storage lowering; it never replaces the
//! original BoundaryCall in the abstract plan. The target origin keeps that
//! distinction available to independent legalization and physical replay.

use crate::lowering::shared::*;

pub(super) fn resolve(
    operation: &AbstractOperation,
    installed: &InstalledProviderCallEvidence,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Result<(AbstractOperation, target_operations::NativeCallOrigin), LoweringError> {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
    } = operation
    else {
        return Err(LoweringError::InstalledProviderCallEvidenceMismatch {
            machine: installed.caller,
            operation: installed.psi_operation,
            boundary: installed.boundary,
        });
    };
    let invalid = || LoweringError::InstalledProviderCallShapeMismatch {
        machine: installed.caller,
        operation: *psi_operation,
        boundary: *boundary,
    };
    let callee = installed.provider.candidate;
    let candidate = functions.get(&callee).copied().ok_or_else(invalid)?;
    let claim_transfers = completion_receipts
        .iter()
        .map(|receipt| terminal_psi::ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
        })
        .collect();
    let resolved = match (result, &candidate.result) {
        (abstract_operations::AbstractBoundaryResult::Unit, AbstractFunctionResult::Unit) => {
            AbstractOperation::CallUnit {
                psi_operation: *psi_operation,
                callee,
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }
        (
            abstract_operations::AbstractBoundaryResult::Structural(result),
            AbstractFunctionResult::Structural(candidate_result),
        ) if result.structural_type == candidate_result.structural_type
            && result.multiplicity == candidate_result.multiplicity
            && result.qualifications == candidate_result.qualifications
            && result.projected_qualifications == candidate_result.projected_qualifications
            && result.claims.is_empty() =>
        {
            AbstractOperation::CallStructural {
                psi_operation: *psi_operation,
                result: result.clone(),
                callee,
                arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                claim_transfers,
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
                selected_evidence: Vec::new(),
            }
        }
        _ => return Err(invalid()),
    };
    let origin = target_operations::NativeCallOrigin::InstalledProvider {
        boundary: *boundary,
        provider: installed.provider.clone(),
        completion_claim_sources: completion_claim_sources.clone(),
        completion_receipts: completion_receipts.clone(),
    };
    Ok((resolved, origin))
}

pub(super) fn retain_origin(
    operations: &mut [TargetUnitOperation],
    installed: &InstalledProviderCallEvidence,
    origin: target_operations::NativeCallOrigin,
) -> Result<(), LoweringError> {
    let [
        TargetUnitOperation::Call {
            psi_operation,
            callee,
            origin: actual,
            ..
        },
    ] = operations
    else {
        return Err(LoweringError::InstalledProviderCallShapeMismatch {
            machine: installed.caller,
            operation: installed.psi_operation,
            boundary: installed.boundary,
        });
    };
    if *psi_operation != installed.psi_operation
        || *callee != installed.provider.candidate
        || *actual != target_operations::NativeCallOrigin::Authored
    {
        return Err(LoweringError::InstalledProviderCallEvidenceMismatch {
            machine: installed.caller,
            operation: installed.psi_operation,
            boundary: installed.boundary,
        });
    }
    *actual = origin;
    Ok(())
}
