use crate::assignment::shared::*;
pub(super) fn assign_normalized_foreign_scalar_call_for_plan(
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    result_home: Option<&omega_target_operations::TargetUnitScalarHomeRequirement>,
    psi_operation: OperationId,
    preceding_operations: &[TargetUnitOperation],
    assigned_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
    native_callback: Option<&omega_target_operations::TargetNativeCallbackArgument>,
) -> Result<Vec<AssignedNormalizedForeignScalarArgument>, AssignmentError> {
    let result_shape = result_home
        .map(|result| {
            let expected_shape = super::super::scalar_call::fixed_integer_shape(
                result.source_value,
                result.scalar_type,
            )
            .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
            if result.shape != expected_shape {
                return Err(AssignmentError::ExpressionStackFrameNotEncodable);
            }
            Ok(expected_shape)
        })
        .transpose()?;
    let callback_ordinal = native_callback
        .map(|callback| usize::try_from(callback.application.native_ordinal))
        .transpose()
        .map_err(|_| AssignmentError::InvalidNativeCallbackArgument(psi_operation))?;
    let signature = CallSignature {
        parameters: boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: result_shape,
    };
    let validated = match native_callback {
        Some(callback) => {
            omega_calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                boundary_entry_plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => omega_calling_conventions::validate_boundary_entry_plan(
            boundary_entry_plan.clone(),
            &signature,
        ),
    }
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    let canonical = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &signature,
    )
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    let mut canonicalized_boundary = boundary_entry_plan.clone();
    canonicalized_boundary
        .call
        .callback_materializations
        .clear();
    if validated.plan() != boundary_entry_plan
        || canonical.plan() != &canonicalized_boundary
        || boundary_entry_plan.call.parameters.len()
            != scalar_arguments.len() + usize::from(native_callback.is_some())
        || match (result_home, boundary_entry_plan.call.result.as_ref()) {
            (None, None) => false,
            (Some(result), Some(placement)) => {
                let exact_register_result = matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    }] if *byte_size == result.shape.byte_size
                );
                result.defining_operation != psi_operation
                    || placement.shape != result.shape
                    || !exact_register_result
            }
            _ => true,
        }
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    let mut assigned = Vec::with_capacity(scalar_arguments.len());
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        let physical_index = parameter_index
            + usize::from(callback_ordinal.is_some_and(|ordinal| parameter_index >= ordinal));
        let source_value = argument.source_value();
        let scalar_type = argument.scalar_type();
        let placed_byte_size = match argument.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ]
            | [
                ValueLocation::Stack {
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] => *byte_size,
            _ => {
                return Err(AssignmentError::ExpressionParameterLocationConflict {
                    value: source_value,
                    parameter_index,
                });
            }
        };
        let expected_bytes = scalar_type.bits().div_ceil(8);
        if scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
            || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
            || u32::try_from(physical_index).ok() != Some(argument.parameter_index)
            || argument.placement != boundary_entry_plan.call.parameters[physical_index]
            || argument.placement.shape
                != ValueShape::integer(expected_bytes, expected_bytes.next_power_of_two().min(8))
            || expected_bytes != placed_byte_size
        {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: source_value,
                parameter_index,
            });
        }
        let source = super::super::scalar_call::assign_known_unit_scalar_source(
            argument.source,
            preceding_operations,
            assigned_homes,
        )
        .ok_or(AssignmentError::ExpressionParameterLocationConflict {
            value: source_value,
            parameter_index,
        })?;
        if let [ValueLocation::Register { register, .. }] = argument.placement.locations.as_slice()
        {
            crate::assignment::placement::require_register_architecture(
                source_value,
                *register,
                target.architecture,
            )?;
        }
        assigned.push(AssignedNormalizedForeignScalarArgument {
            parameter_index: argument.parameter_index,
            source,
            placement: argument.placement.clone(),
        });
    }
    Ok(assigned)
}

#[cfg(test)]
pub(super) fn assign_normalized_foreign_scalar_arguments_for_plan(
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    preceding_operations: &[TargetUnitOperation],
    assigned_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Result<Vec<AssignedNormalizedForeignScalarArgument>, AssignmentError> {
    assign_normalized_foreign_scalar_call_for_plan(
        boundary_entry_plan,
        target,
        scalar_arguments,
        None,
        OperationId::new(1).expect("one is a valid operation identity"),
        preceding_operations,
        assigned_homes,
        None,
    )
}
