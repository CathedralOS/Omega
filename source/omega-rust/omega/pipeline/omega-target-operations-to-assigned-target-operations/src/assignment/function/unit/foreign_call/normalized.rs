use crate::assignment::shared::*;

pub(super) fn assign_normalized_foreign_scalar_arguments_for_plan(
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    preceding_operations: &[TargetUnitOperation],
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, AssignmentError> {
    if scalar_arguments.len() > 5 {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    let signature = CallSignature {
        parameters: scalar_arguments
            .iter()
            .map(|argument| argument.placement.shape)
            .collect(),
        result: None,
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
        || boundary_entry_plan.call.result.is_some()
        || boundary_entry_plan.call.parameters.len() != scalar_arguments.len()
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = argument.placement.locations.as_slice()
        else {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: argument.source_value,
                parameter_index,
            });
        };
        let expected_bytes = argument.scalar_type.bits().div_ceil(8);
        if argument.scalar_type.carrier() != psi_core::IntegerCarrier::Fixed
            || !matches!(argument.scalar_type.bits(), 8 | 16 | 32 | 64)
            || argument.parameter_index != parameter_index as u32
            || argument.placement != boundary_entry_plan.call.parameters[parameter_index]
            || argument.placement.shape
                != ValueShape::integer(expected_bytes, expected_bytes.next_power_of_two().min(8))
            || u16::try_from(expected_bytes) != Ok(*byte_size)
            || psi_core::ScalarTerm::integer(argument.scalar_type, argument.immediate).is_err()
        {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: argument.source_value,
                parameter_index,
            });
        }
        let matching_constants = preceding_operations
            .iter()
            .filter_map(|operation| match operation {
                TargetUnitOperation::IntegerConstant {
                    result,
                    scalar_type,
                    value,
                    ..
                } if *result == argument.source_value => Some((*scalar_type, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if matching_constants.as_slice() != [(argument.scalar_type, argument.immediate)] {
            return Err(AssignmentError::ExpressionParameterLocationConflict {
                value: argument.source_value,
                parameter_index,
            });
        }
        crate::assignment::placement::require_register_architecture(
            argument.source_value,
            *register,
            target.architecture,
        )?;
    }
    Ok(scalar_arguments.to_vec())
}
