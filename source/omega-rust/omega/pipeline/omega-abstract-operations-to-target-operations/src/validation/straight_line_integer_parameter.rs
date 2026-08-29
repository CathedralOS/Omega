use std::collections::BTreeSet;

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractParameter, AbstractResult,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_target_operations::{ScalarParameterLocation, TargetFunction, TargetOperation};
use psi_core::ScalarType;

use super::{
    StraightLineIntegerParameterTranslationError, StraightLineIntegerParameterTranslationReceipt,
};

pub(super) fn is_candidate(function: &AbstractFunction) -> bool {
    !function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && matches!(
            function.result,
            AbstractFunctionResult::Scalar(AbstractResult {
                scalar_type: ScalarType::Integer(_),
                ..
            })
        )
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
        && matches!(
            function.operations.as_slice(),
            [AbstractOperation::Return {
                cleanup_actions,
                ..
            }] if cleanup_actions.is_empty()
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerParameterTranslationReceipt,
    StraightLineIntegerParameterTranslationError,
> {
    if source.parameters.is_empty() {
        return Err(StraightLineIntegerParameterTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLineIntegerParameterTranslationError::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        value: function_result,
        scalar_type: ScalarType::Integer(result_type),
    }) = source.result
    else {
        return Err(StraightLineIntegerParameterTranslationError::SourceResult);
    };
    if !source.entry_claims.is_empty() {
        return Err(StraightLineIntegerParameterTranslationError::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(StraightLineIntegerParameterTranslationError::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineIntegerParameterTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLineIntegerParameterTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerParameterTranslationError::SourceCleanup);
    }
    if *result != function_result || *scalar_type != ScalarType::Integer(result_type) {
        return Err(StraightLineIntegerParameterTranslationError::SourceReturnLink);
    }

    let mut parameter_values = BTreeSet::new();
    if source
        .parameters
        .iter()
        .any(|parameter| !parameter_values.insert(parameter.value))
    {
        return Err(StraightLineIntegerParameterTranslationError::SourceParameterRoster);
    }
    let Some(parameter_index) = source
        .parameters
        .iter()
        .position(|parameter| parameter.value == *value)
    else {
        return Err(StraightLineIntegerParameterTranslationError::SourceReturnLink);
    };
    let returned_parameter = &source.parameters[parameter_index];
    if returned_parameter.scalar_type != ScalarType::Integer(result_type) {
        return Err(StraightLineIntegerParameterTranslationError::SourceReturnLink);
    }

    let parameter_shapes = source
        .parameters
        .iter()
        .map(parameter_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let expected_bytes = parameter_shapes[parameter_index].byte_size;
    let signature = CallSignature {
        parameters: parameter_shapes,
        result: Some(scalar_shape(ScalarType::Integer(result_type))),
    };
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &signature,
    )
    .map_err(|_| StraightLineIntegerParameterTranslationError::AbiPlan)?;
    if call_plan.parameters.len() != source.parameters.len() {
        return Err(StraightLineIntegerParameterTranslationError::AbiParameterCount);
    }
    let location = parameter_location(&call_plan.parameters[parameter_index], expected_bytes)?;

    if !target.provenance.operations.is_empty() || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLineIntegerParameterTranslationError::TargetProvenance);
    }
    if !matches!(
        target.operation,
        TargetOperation::ReturnIntegerParameter {
            psi_edge: target_edge,
            source_value,
            scalar_type: target_type,
            parameter_index: target_index,
            location: target_location,
        } if target_edge == *psi_edge
            && source_value == *value
            && target_type == result_type
            && target_index == parameter_index
            && target_location == location
    ) {
        return Err(StraightLineIntegerParameterTranslationError::TargetOperation);
    }
    Ok(StraightLineIntegerParameterTranslationReceipt::new(
        source.machine,
        *psi_edge,
        *value,
        result_type,
        parameter_index,
        location,
    ))
}

fn parameter_shape(
    parameter: &AbstractParameter,
) -> Result<ValueShape, StraightLineIntegerParameterTranslationError> {
    if matches!(
        parameter.scalar_type,
        ScalarType::Integer(integer_type) if !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    ) {
        return Err(StraightLineIntegerParameterTranslationError::SourceParameterShape);
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
) -> Result<ScalarParameterLocation, StraightLineIntegerParameterTranslationError> {
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
        _ => Err(StraightLineIntegerParameterTranslationError::AbiParameterPlacement),
    }
}
