//! Input predicates for a genuine single-block scalar return.

use abstract_operations::{AbstractFunction, AbstractFunctionResult};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueLocation, ValueShape, evaluate_call_plan,
};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target_operations::{
    FixedIntegerScalarFunctionAbi, ScalarParameterLocation, TargetFunction, TargetIntegerControl,
    TargetIntegerExpression, TargetOperation,
};

use super::LegalizationError;

/// This is a lossless vocabulary projection, not legalization or evidence
/// construction. Both independent algorithms inspect the same source leaf.
pub(super) fn control(target: &TargetFunction) -> Option<(IntegerType, TargetIntegerControl)> {
    let (scalar_type, psi_return_edge, source_value, expression) = match &target.operation {
        TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } => (
            *scalar_type,
            *psi_edge,
            *source_value,
            TargetIntegerExpression::Immediate {
                source_value: *source_value,
                value: *value,
            },
        ),
        TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => (
            *scalar_type,
            *psi_edge,
            *source_value,
            TargetIntegerExpression::Parameter {
                source_value: *source_value,
                parameter_index: *parameter_index,
                location: *location,
            },
        ),
        _ => return None,
    };
    Some((
        scalar_type,
        TargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression,
        },
    ))
}

pub(super) fn validate_input<'a>(
    function: usize,
    native_target: target::NativeTarget,
    target: &'a TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
) -> Result<&'a FixedIntegerScalarFunctionAbi, LegalizationError> {
    let invalid = || LegalizationError::UnsupportedSourceShape { function };
    let (integer, control) = control(target).ok_or_else(invalid)?;
    if integer != IntegerType::new(IntegerSign::Unsigned, 64).expect("u64") {
        return Err(LegalizationError::UnsupportedIntegerShape { function });
    }
    let scalar_type = ScalarType::Integer(integer);
    let abi = target
        .fixed_integer_scalar_abi
        .as_ref()
        .ok_or_else(invalid)?;
    let AbstractFunctionResult::Scalar(result) = abstracted.result else {
        return Err(invalid());
    };
    let [entry] = abstracted.block_entries.as_slice() else {
        return Err(invalid());
    };
    let [block] = optimized.blocks.as_slice() else {
        return Err(invalid());
    };
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment.is_some()
        || target.mixed_structural_scalar_abi.is_some()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || abstracted.entry != entry.block
        || optimized.entry != entry.block
        || block.id != entry.block
        || entry.operation_offset != 0
        || !entry.parameters.is_empty()
        || !block.parameters.is_empty()
        || result.scalar_type != scalar_type
        || abi.result.scalar_type != integer
        || abi.result.value != result.value
        || abi.parameters.len() != abstracted.parameters.len()
        || abi.parameters.len() != optimized.parameters.len()
        || abi
            .parameters
            .iter()
            .zip(&abstracted.parameters)
            .zip(&optimized.parameters)
            .any(|((abi, declared), optimized)| {
                abi.value != declared.value
                    || abi.value != optimized.value
                    || abi.scalar_type != integer
                    || declared.scalar_type != scalar_type
                    || optimized.scalar_type != scalar_type
            })
    {
        return Err(invalid());
    }
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(native_target),
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); abi.parameters.len()],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .map_err(|_| invalid())?;
    if abi.call_plan != call_plan
        || abi.result.placement != *call_plan.result.as_ref().ok_or_else(invalid)?
        || abi
            .parameters
            .iter()
            .zip(&call_plan.parameters)
            .any(|(value, placement)| value.placement != *placement)
    {
        return Err(invalid());
    }
    if let TargetIntegerControl::Return {
        expression:
            TargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            },
        ..
    } = control
    {
        let parameter = abi.parameters.get(parameter_index).ok_or_else(invalid)?;
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            },
        ] = parameter.placement.locations.as_slice()
        else {
            return Err(invalid());
        };
        if parameter.value != source_value
            || location != ScalarParameterLocation::Register(*register)
        {
            return Err(invalid());
        }
    }
    Ok(abi)
}
