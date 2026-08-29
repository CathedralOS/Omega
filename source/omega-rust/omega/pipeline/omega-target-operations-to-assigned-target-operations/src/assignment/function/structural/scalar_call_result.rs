//! Assignment for the bounded whole-root structural-call/scalar-return ABI.

use omega_assigned_target_operations::{AssignedAggregateCopy, AssignedOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::TargetOperation;
use psi_core::MachineId;

use crate::AssignmentError;

pub(crate) fn assign(
    machine: MachineId,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let TargetOperation::ReturnStructuralScalarCall {
        psi_edge,
        psi_operation,
        source_value,
        scalar_type,
        callee,
        structural_types,
        call_plan,
        structural_parameters,
        arguments,
        claim_transfers,
    } = operation
    else {
        unreachable!("structural scalar assignment receives its exact target carrier")
    };
    let result_bytes = match scalar_type {
        psi_core::ScalarType::Boolean => 1,
        psi_core::ScalarType::Integer(integer) => integer.bits().div_ceil(8),
    };
    let result_shape = ValueShape::integer(result_bytes, result_bytes.next_power_of_two().min(8));
    let expected_caller_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: structural_parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| AssignmentError::UnsupportedScalarCleanup(machine))?;
    let expected_callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: arguments.iter().map(|argument| argument.shape).collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| AssignmentError::UnsupportedScalarCleanup(machine))?;
    if arguments.is_empty()
        || *call_plan != expected_caller_plan
        || arguments.len() != structural_parameters.len()
        || arguments
            .iter()
            .zip(structural_parameters)
            .zip(&expected_callee_plan.parameters)
            .any(|((argument, parameter), destination)| {
                !argument.path.is_empty()
                    || argument.place != parameter.place
                    || argument.root_structural_type != parameter.structural_type
                    || argument.structural_type != parameter.structural_type
                    || argument.shape != parameter.shape
                    || argument.source_byte_offset != 0
                    || argument.fixed_array_length.is_some()
                    || argument.element_stride.is_some()
                    || argument.source != parameter.placement
                    || argument.destination != *destination
            })
    {
        return Err(AssignmentError::UnsupportedScalarCleanup(machine));
    }
    Ok(AssignedOperation::ReturnStructuralScalarCall {
        psi_edge: *psi_edge,
        psi_operation: *psi_operation,
        source_value: *source_value,
        scalar_type: *scalar_type,
        callee: *callee,
        structural_types: structural_types.clone(),
        call_plan: call_plan.clone(),
        structural_parameters: structural_parameters.clone(),
        copies: arguments
            .iter()
            .map(|argument| AssignedAggregateCopy {
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
            })
            .collect(),
        claim_transfers: claim_transfers.clone(),
    })
}
