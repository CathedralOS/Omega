use omega_abstract_operations::AbstractParameter;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_target_operations::ScalarParameterLocation;
use psi_core::ScalarType;

use super::super::model::StraightLineParameterReconstructionError;

pub(super) fn replay(
    parameters: &[AbstractParameter],
    result_type: ScalarType,
    expected_target: NativeTarget,
) -> Result<Vec<ScalarParameterLocation>, StraightLineParameterReconstructionError> {
    let parameter_shapes = parameters
        .iter()
        .map(parameter_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: parameter_shapes.clone(),
        result: Some(scalar_shape(result_type)),
    };
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &signature,
    )
    .map_err(|_| StraightLineParameterReconstructionError::AbiPlan)?;
    if call_plan.parameters.len() != parameters.len() {
        return Err(StraightLineParameterReconstructionError::AbiParameterCount);
    }
    call_plan
        .parameters
        .iter()
        .zip(parameter_shapes)
        .map(|(placement, shape)| parameter_location(placement, shape.byte_size))
        .collect()
}

fn parameter_shape(
    parameter: &AbstractParameter,
) -> Result<ValueShape, StraightLineParameterReconstructionError> {
    if matches!(
        parameter.scalar_type,
        ScalarType::Integer(integer_type) if !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    ) {
        return Err(StraightLineParameterReconstructionError::SourceParameterShape);
    }
    Ok(scalar_shape(parameter.scalar_type))
}

fn scalar_shape(scalar_type: ScalarType) -> ValueShape {
    let bytes = match scalar_type {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer_type) => integer_type.bits().div_ceil(8),
    };
    ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
}

fn parameter_location(
    placement: &ValuePlacement,
    expected_bytes: u16,
) -> Result<ScalarParameterLocation, StraightLineParameterReconstructionError> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::Register(*register)),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        }),
        _ => Err(StraightLineParameterReconstructionError::AbiParameterPlacement),
    }
}
