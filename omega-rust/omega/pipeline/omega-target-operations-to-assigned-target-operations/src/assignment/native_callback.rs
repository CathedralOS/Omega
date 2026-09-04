use std::collections::BTreeSet;

use omega_assigned_target_operations::{AssignedCallDestination, AssignedNativeCallbackArgument};
use omega_calling_conventions::{CallSignature, ValueLocation};
use omega_target_operations::{
    TargetNativeCallbackArgument, TargetOperation, TargetOperationPlan, TargetUnitOperation,
};

use crate::AssignmentError;

pub(super) fn assign(
    plan: &TargetOperationPlan,
    callbacks: &[TargetNativeCallbackArgument],
) -> Result<Vec<AssignedNativeCallbackArgument>, AssignmentError> {
    if callbacks.len() > 1 {
        return Err(AssignmentError::MultipleNativeCallbackArguments);
    }
    let mut operations = BTreeSet::new();
    let mut placement_indices = BTreeSet::new();
    let mut assigned = Vec::with_capacity(callbacks.len());
    for callback in callbacks {
        if !operations.insert(callback.terminal_operation)
            || !placement_indices.insert(callback.placement_index)
        {
            return Err(AssignmentError::DuplicateNativeCallbackArgument(
                callback.terminal_operation,
            ));
        }
        let matches = plan
            .functions
            .iter()
            .filter_map(|function| match &function.operation {
                TargetOperation::UnitBody(body) => Some(body),
                _ => None,
            })
            .flat_map(|body| &body.operations)
            .filter_map(|operation| match operation {
                TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    binding,
                    scalar_arguments,
                    ..
                } if *psi_operation == callback.terminal_operation => {
                    Some((binding, scalar_arguments))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(binding, scalar_arguments)] = matches.as_slice() else {
            return Err(AssignmentError::UnknownNativeCallbackArgument(
                callback.terminal_operation,
            ));
        };
        if !callback_is_exact(plan, callback, binding, scalar_arguments) {
            return Err(AssignmentError::InvalidNativeCallbackArgument(
                callback.terminal_operation,
            ));
        }
        let destination = destination(plan, callback).ok_or(
            AssignmentError::InvalidNativeCallbackArgument(callback.terminal_operation),
        )?;
        assigned.push(AssignedNativeCallbackArgument {
            target: callback.clone(),
            destination,
        });
    }
    Ok(assigned)
}

fn callback_is_exact(
    plan: &TargetOperationPlan,
    callback: &TargetNativeCallbackArgument,
    binding: &omega_target_operations::NormalizedForeignCallBinding,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
) -> bool {
    let Ok(callback_ordinal) = usize::try_from(callback.application.native_ordinal) else {
        return false;
    };
    let parameters = &callback.registrar_boundary_entry_plan.call.parameters;
    let signature = CallSignature {
        parameters: parameters.iter().map(|placement| placement.shape).collect(),
        result: callback
            .registrar_boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|result| result.shape),
    };
    let validated =
        omega_calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            callback.registrar_boundary_entry_plan.clone(),
            &signature,
            &callback.registrar_context,
        );
    let ([binder], [demand], [materialization]) = (
        callback.registrar_context.binders.as_slice(),
        callback.registrar_context.demands.as_slice(),
        callback
            .registrar_boundary_entry_plan
            .call
            .callback_materializations
            .as_slice(),
    ) else {
        return false;
    };
    callback.callback_function.is_valid()
        && callback.callback_function.callback_thunk_placement_index()
            == Some(callback.placement_index)
        && callback.registrar_application_commitment != [0; 32]
        && binding.locator.target().native_target() == plan.target
        && binding.boundary_entry_plan == callback.registrar_boundary_entry_plan
        && validated
            .as_ref()
            .is_ok_and(|validated| validated.plan() == &callback.registrar_boundary_entry_plan)
        && callback.application.shape == callback.application.placement.shape
        && parameters.get(callback_ordinal) == Some(&callback.application.placement)
        && demand.destination
            == omega_calling_conventions::NativePlace::Parameter(callback.application.parameter)
        && materialization.destination == demand.destination
        && materialization.binder == binder.binder
        && binder.requirement == demand.requirement
        && parameters.len() == scalar_arguments.len() + 1
        && scalar_arguments
            .iter()
            .enumerate()
            .all(|(semantic_index, argument)| {
                let physical_index =
                    semantic_index + usize::from(semantic_index >= callback_ordinal);
                u32::try_from(physical_index).ok() == Some(argument.parameter_index)
                    && parameters.get(physical_index) == Some(&argument.placement)
            })
}

fn destination(
    plan: &TargetOperationPlan,
    callback: &TargetNativeCallbackArgument,
) -> Option<AssignedCallDestination> {
    let size = u16::try_from(plan.target.pointer_size).ok()?;
    let alignment = u16::try_from(plan.target.pointer_alignment).ok()?;
    if alignment == 0 {
        return None;
    }
    if callback.application.shape != omega_calling_conventions::ValueShape::integer(size, alignment)
    {
        return None;
    }
    match callback.application.placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == size && register.architecture() == plan.target.architecture => {
            Some(AssignedCallDestination::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                alignment: placed_alignment,
            },
        ] if *byte_size == size
            && *placed_alignment == alignment
            && stack_byte_offset % u32::from(alignment) == 0 =>
        {
            Some(AssignedCallDestination::OutgoingStack {
                byte_offset: *stack_byte_offset,
            })
        }
        _ => None,
    }
}
