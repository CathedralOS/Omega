use crate::assignment::shared::*;
pub(super) fn assign_normalized_foreign_scalar_call_for_plan(
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    result_home: Option<&omega_target_operations::TargetUnitScalarHomeRequirement>,
    psi_operation: OperationId,
    preceding_operations: &[TargetUnitOperation],
    assigned_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
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
    let signature = CallSignature {
        parameters: scalar_arguments
            .iter()
            .map(|argument| argument.placement.shape)
            .collect(),
        result: result_shape,
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    let canonical = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &signature,
    )
    .map_err(|_| AssignmentError::ExpressionStackFrameNotEncodable)?;
    if validated.plan() != boundary_entry_plan
        || canonical.plan() != boundary_entry_plan
        || boundary_entry_plan.call.parameters.len() != scalar_arguments.len()
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
        let source_value = argument.source_value();
        let scalar_type = argument.scalar_type();
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = argument.placement.locations.as_slice()
        else {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: source_value,
                parameter_index,
            });
        };
        let expected_bytes = scalar_type.bits().div_ceil(8);
        if scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
            || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
            || argument.parameter_index != parameter_index as u32
            || argument.placement != boundary_entry_plan.call.parameters[parameter_index]
            || argument.placement.shape
                != ValueShape::integer(expected_bytes, expected_bytes.next_power_of_two().min(8))
            || expected_bytes != *byte_size
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
        crate::assignment::placement::require_register_architecture(
            source_value,
            *register,
            target.architecture,
        )?;
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
    )
}
