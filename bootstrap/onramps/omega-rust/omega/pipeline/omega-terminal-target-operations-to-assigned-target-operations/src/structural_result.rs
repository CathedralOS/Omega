//! Assignment for the bounded whole-root structural-call/result carrier.

use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_terminal_assigned_target_operations::{
    TerminalAssignedAggregateCopy, TerminalAssignedOperation,
};
use omega_terminal_target_operations::TerminalTargetOperation;
use psi_core::MachineId;

use super::AssignmentError;

pub(super) fn assign(
    _machine: MachineId,
    operation: &TerminalTargetOperation,
    target: NativeTarget,
) -> Result<TerminalAssignedOperation, AssignmentError> {
    let TerminalTargetOperation::ReturnStructuralCall {
        psi_edge,
        psi_operation,
        operation_result,
        result,
        callee,
        structural_types,
        call_plan,
        callee_call_plan,
        structural_parameters,
        arguments,
        claim_transfers,
        returned_claim_transfers,
        returned_claims,
    } = operation
    else {
        unreachable!("structural-result assignment receives its exact carrier")
    };
    let [parameter] = structural_parameters.as_slice() else {
        return Err(AssignmentError::UnsupportedStructuralPlacement(
            operation_result.place,
        ));
    };
    let [argument] = arguments.as_slice() else {
        return Err(AssignmentError::UnsupportedStructuralPlacement(
            parameter.place,
        ));
    };
    let expected_caller = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![parameter.shape],
            result: Some(parameter.shape),
        },
    )
    .map_err(|_| AssignmentError::UnsupportedStructuralPlacement(parameter.place))?;
    let expected_callee = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![argument.shape],
            result: Some(argument.shape),
        },
    )
    .map_err(|_| AssignmentError::UnsupportedStructuralPlacement(parameter.place))?;
    if *call_plan != expected_caller
        || *callee_call_plan != expected_callee
        || parameter.shape.byte_size != 8
        || parameter.shape.alignment != 8
        || argument.path.len() != 0
        || argument.place != parameter.place
        || argument.root_structural_type != parameter.structural_type
        || argument.structural_type != parameter.structural_type
        || argument.shape != parameter.shape
        || argument.source_byte_offset != 0
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || argument.source != parameter.placement
        || expected_callee.parameters.as_slice() != [argument.destination.clone()]
        || operation_result.structural_type != result.structural_type
        || operation_result.multiplicity != result.multiplicity
        || operation_result.qualifications != result.qualifications
        || claim_transfers.len() != 1
        || returned_claim_transfers.len() != 1
        || returned_claims.len() != 1
    {
        return Err(AssignmentError::UnsupportedStructuralPlacement(
            parameter.place,
        ));
    }
    Ok(TerminalAssignedOperation::ReturnStructuralCall {
        psi_edge: *psi_edge,
        psi_operation: *psi_operation,
        operation_result: operation_result.clone(),
        result: result.clone(),
        callee: *callee,
        structural_types: structural_types.clone(),
        call_plan: call_plan.clone(),
        callee_call_plan: callee_call_plan.clone(),
        structural_parameters: structural_parameters.clone(),
        copies: vec![TerminalAssignedAggregateCopy {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: argument.root_structural_type,
            structural_type: argument.structural_type,
            shape: argument.shape,
            source_byte_offset: argument.source_byte_offset,
            fixed_array_length: argument.fixed_array_length,
            element_stride: argument.element_stride,
            source: argument.source.clone(),
            destination: argument.destination.clone(),
        }],
        claim_transfers: claim_transfers.clone(),
        returned_claim_transfers: returned_claim_transfers.clone(),
        returned_claims: returned_claims.clone(),
    })
}
