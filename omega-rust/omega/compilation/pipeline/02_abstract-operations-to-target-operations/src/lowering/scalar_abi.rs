use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use calling_conventions::ValueShape;
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use semantic_vocabulary::{IeeeFloatFormat, IntegerType, ScalarType};
use target::NativeTarget;
use target_operations::{MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi};

// Service ceilings authorize effects; parameter and result layouts determine ABI.
// Terminal verification checks reach independently of this derived publication.
pub(super) fn derive_mixed_structural_scalar_function_abi(
    function: &AbstractFunction,
    target: NativeTarget,
    structural_types: &StructuralTypeLookup<'_>,
) -> Result<Option<MixedStructuralScalarFunctionAbi>, LoweringError> {
    if function.structural_parameters.is_empty() {
        return Ok(None);
    }
    let Some(result) = function.result.scalar() else {
        return Ok(None);
    };
    if fixed_native_scalar_shape(result.scalar_type).is_none() {
        return Ok(None);
    }
    // Native scalar eligibility and ordinary signature placement do not
    // change when structural parameters appear beside the scalar lanes.
    let Ok(prepared) =
        super::function_signature::prepare_function_signature(function, target, structural_types)
    else {
        // ABI publication is a derived claim, not a new lowering entrance.
        // The ordinary function lowerer retains authority for diagnostics.
        return Ok(None);
    };
    let result_placement = prepared.call_plan.result.clone().ok_or(
        LoweringError::FixedIntegerScalarAbiPlanMissingResult(function.machine),
    )?;
    Ok(Some(MixedStructuralScalarFunctionAbi {
        call_plan: prepared.call_plan,
        scalar_parameters: prepared.scalar_parameters,
        structural_parameters: prepared.parameters,
        result: ScalarAbiValue {
            value: result.value,
            scalar_type: result.scalar_type,
            placement: result_placement,
        },
    }))
}

pub(super) fn derive_fixed_scalar_function_abi(
    function: &AbstractFunction,
    target: NativeTarget,
) -> Result<Option<ScalarFunctionAbi>, LoweringError> {
    if !function.structural_parameters.is_empty() || !function.entry_claims.is_empty() {
        return Ok(None);
    }
    let Some(result) = function.result.scalar() else {
        return Ok(None);
    };
    let Some(result_shape) = fixed_native_scalar_shape(result.scalar_type) else {
        return Ok(None);
    };

    let parameter_types_and_shapes = function
        .parameters
        .iter()
        .map(|parameter| {
            let scalar_type = parameter.scalar_type;
            Some((scalar_type, fixed_native_scalar_shape(scalar_type)?))
        })
        .collect::<Option<Vec<_>>>();
    let Some(parameter_types_and_shapes) = parameter_types_and_shapes else {
        return Ok(None);
    };
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_types_and_shapes
                .iter()
                .map(|(_, shape)| *shape)
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let result_placement =
        call_plan
            .result
            .clone()
            .ok_or(LoweringError::FixedIntegerScalarAbiPlanMissingResult(
                function.machine,
            ))?;
    if call_plan.parameters.len() != function.parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    let parameters = function
        .parameters
        .iter()
        .zip(parameter_types_and_shapes)
        .zip(&call_plan.parameters)
        .map(|((parameter, (scalar_type, expected_shape)), placement)| {
            if placement.shape != expected_shape {
                return Err(LoweringError::UnsupportedScalarParameterPlacement(
                    parameter.value,
                ));
            }
            Ok(ScalarAbiValue {
                value: parameter.value,
                scalar_type,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if result_placement.shape != result_shape {
        return Err(LoweringError::FixedIntegerScalarAbiPlanMissingResult(
            function.machine,
        ));
    }
    Ok(Some(ScalarFunctionAbi {
        call_plan,
        parameters,
        result: ScalarAbiValue {
            value: result.value,
            scalar_type: result.scalar_type,
            placement: result_placement,
        },
    }))
}

pub(super) fn fixed_native_scalar_shape(scalar_type: ScalarType) -> Option<ValueShape> {
    match scalar_type {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) => fixed_native_integer_shape(integer),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Some(ValueShape::float(4)),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Some(ValueShape::float(8)),
    }
}
pub(super) fn fixed_native_integer_shape(scalar_type: IntegerType) -> Option<ValueShape> {
    // Hosted address models currently use one 64-bit scalar transport. Keep
    // the address carrier in every value record; an integer-class ABI slot
    // does not convert it to a count or admit other address models.
    if scalar_type.is_address() && scalar_type.bits() == 64 {
        return Some(ValueShape::integer(8, 8));
    }
    if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let bytes = scalar_type.bits().div_ceil(8);
    Some(ValueShape::integer(
        u32::from(bytes),
        bytes.next_power_of_two().min(8),
    ))
}
