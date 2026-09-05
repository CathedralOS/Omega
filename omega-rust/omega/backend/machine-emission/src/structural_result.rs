//! Emission adapter for the bounded whole-root structural-call/result carrier.

use assigned_target_operations::{
    AssignedFunction, AssignedOperation, AssignedUnitBody, AssignedUnitOperation,
};
use calling_conventions::{ValueClass, ValueLocation, ValuePlacement};
use machine_code::InternalStructuralCallResult;
use target::NativeTarget;

use super::{EmissionError, unit::UnitEmission, unit::emit_unit_body};

pub(super) fn emit(
    operation: &AssignedOperation,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<UnitEmission, EmissionError> {
    let AssignedOperation::ReturnStructuralCall {
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
        requirement_obligations,
        crash_continuations,
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
        || !direct_fragments(&caller_result_placement)
        || !direct_fragments(&callee_result_placement)
        || operation_result.structural_type != result.structural_type
        || operation_result.multiplicity != result.multiplicity
        || operation_result.qualifications != result.qualifications
        || copies.len() != 1
        || claim_transfers.len() != 1
        || returned_claim_transfers.len() != 1
        || returned_claims.len() != 1
        || copies.first().is_none_or(|copy| {
            !direct_fragments(&copy.source) || !direct_fragments(&copy.destination)
        })
    {
        return Err(EmissionError::UnsupportedStructuralReturnPlacement(
            operation_result.place,
        ));
    }
    let mut emitted = emit_unit_body(
        &AssignedUnitBody {
            structural_types: structural_types.clone(),
            call_plan: call_plan.clone(),
            scalar_parameters: Vec::new(),
            parameters: structural_parameters.clone(),
            operations: vec![
                AssignedUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    result: None,
                    call_plan: callee_call_plan.clone(),
                    scalar_arguments: Vec::new(),
                    copies: copies.clone(),
                    claim_transfers: claim_transfers.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    crash_continuations: crash_continuations.clone(),
                },
                AssignedUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        None,
        None,
        target,
        functions,
        &[],
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
    call.structural_result = Some(InternalStructuralCallResult {
        operation_result: operation_result.clone(),
        function_result: result.clone(),
        returned_claim_transfers: returned_claim_transfers.clone(),
        returned_claims: returned_claims.clone(),
        caller_result_placement,
        callee_result_placement,
    });
    Ok(emitted)
}

fn direct_fragments(placement: &ValuePlacement) -> bool {
    if placement.shape.class != ValueClass::Integer
        || !((placement.shape.byte_size == 8 && placement.shape.alignment == 8)
            || (9..=16).contains(&placement.shape.byte_size))
        || !(1..=2).contains(&placement.locations.len())
    {
        return false;
    }
    let mut expected_offset = 0_u16;
    for location in &placement.locations {
        let ValueLocation::Register {
            value_byte_offset,
            byte_size,
            ..
        } = *location
        else {
            return false;
        };
        let expected_size = (placement.shape.byte_size - expected_offset).min(8);
        if value_byte_offset != expected_offset || byte_size != expected_size {
            return false;
        }
        let Some(next) = expected_offset.checked_add(byte_size) else {
            return false;
        };
        expected_offset = next;
    }
    expected_offset == placement.shape.byte_size
}
