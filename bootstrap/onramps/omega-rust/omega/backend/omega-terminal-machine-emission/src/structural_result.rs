//! Emission adapter for the bounded whole-root structural-call/result carrier.

use omega_target::NativeTarget;
use omega_terminal_assigned_target_operations::{
    TerminalAssignedFunction, TerminalAssignedOperation, TerminalAssignedUnitBody,
    TerminalAssignedUnitOperation,
};
use omega_terminal_machine_code::TerminalInternalStructuralCallResult;

use super::{EmissionError, unit::UnitEmission, unit::emit_unit_body};

pub(super) fn emit(
    operation: &TerminalAssignedOperation,
    target: NativeTarget,
    functions: &[TerminalAssignedFunction],
) -> Result<UnitEmission, EmissionError> {
    let TerminalAssignedOperation::ReturnStructuralCall {
        psi_edge,
        psi_operation,
        operation_result,
        result,
        callee,
        structural_types,
        call_plan,
        callee_call_plan,
        structural_parameters,
        copies,
        claim_transfers,
        returned_claim_transfers,
        returned_claims,
    } = operation
    else {
        unreachable!("structural-result emission receives its exact carrier")
    };
    let caller_result_placement =
        call_plan
            .result
            .clone()
            .ok_or(EmissionError::UnsupportedStructuralReturnPlacement(
                operation_result.place,
            ))?;
    let callee_result_placement = callee_call_plan.result.clone().ok_or(
        EmissionError::UnsupportedStructuralReturnPlacement(operation_result.place),
    )?;
    if caller_result_placement != callee_result_placement
        || operation_result.structural_type != result.structural_type
        || operation_result.multiplicity != result.multiplicity
        || operation_result.qualifications != result.qualifications
        || copies.len() != 1
        || claim_transfers.len() != 1
        || returned_claim_transfers.len() != 1
        || returned_claims.len() != 1
    {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(
            operation_result.place,
        ));
    }
    let mut emitted = emit_unit_body(
        &TerminalAssignedUnitBody {
            structural_types: structural_types.clone(),
            call_plan: call_plan.clone(),
            parameters: structural_parameters.clone(),
            operations: vec![
                TerminalAssignedUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    result: None,
                    copies: copies.clone(),
                    claim_transfers: claim_transfers.clone(),
                },
                TerminalAssignedUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        target,
        functions,
    )?;
    let [call] = emitted.internal_unit_calls.as_mut_slice() else {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(
            operation_result.place,
        ));
    };
    if call.result.is_some() || call.structural_result.is_some() {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(
            operation_result.place,
        ));
    }
    call.structural_result = Some(TerminalInternalStructuralCallResult {
        operation_result: operation_result.clone(),
        function_result: result.clone(),
        returned_claim_transfers: returned_claim_transfers.clone(),
        returned_claims: returned_claims.clone(),
        caller_result_placement,
        callee_result_placement,
    });
    Ok(emitted)
}
