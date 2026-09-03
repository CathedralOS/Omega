//! ABI planning and structural-parameter preparation for an attached Unit body.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::structural_shape;

pub(super) struct PreparedUnitFunction {
    pub(super) call_plan: CallPlan,
    pub(super) scalar_parameters: Vec<UnitScalarAbiValue>,
    pub(super) parameters: Vec<TargetStructuralParameter>,
}

pub(super) fn prepare_unit_function(
    function: &AbstractFunction,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<PreparedUnitFunction, LoweringError> {
    let scalar_parameter_shapes = function
        .parameters
        .iter()
        .map(|parameter| match parameter.scalar_type {
            ScalarType::Boolean => Ok(ValueShape::integer(1, 1)),
            ScalarType::Integer(scalar_type) => fixed_native_integer_shape(scalar_type).ok_or(
                LoweringError::UnitFunctionHasScalarParameters(function.machine),
            ),
            ScalarType::IeeeFloat(_) => Err(LoweringError::UnitFunctionHasScalarParameters(
                function.machine,
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let parameter_shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| -> Result<ValueShape, LoweringError> {
            let shape = structural_shape(
                parameter.structural_type,
                structural_types,
                &mut shape_cache,
                &mut active,
            )?;
            Ok(
                if parameter.access == psi_terminal::StructuralAccess::MutableBorrow {
                    ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
                } else {
                    shape
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: scalar_parameter_shapes
            .iter()
            .chain(&parameter_shapes)
            .copied()
            .collect(),
        result: None,
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    let expected_parameter_count = function.parameters.len() + function.structural_parameters.len();
    if call_plan.parameters.len() != expected_parameter_count {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: expected_parameter_count,
            actual: call_plan.parameters.len(),
        });
    }
    let scalar_parameters = function
        .parameters
        .iter()
        .zip(&scalar_parameter_shapes)
        .zip(&call_plan.parameters)
        .map(|((parameter, expected_shape), placement)| {
            if placement.shape != *expected_shape {
                return Err(LoweringError::UnsupportedScalarParameterPlacement(
                    parameter.value,
                ));
            }
            Ok(UnitScalarAbiValue {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let parameters = function
        .structural_parameters
        .iter()
        .zip(parameter_shapes)
        .zip(call_plan.parameters.iter().skip(function.parameters.len()))
        .map(
            |((parameter, shape), placement)| TargetStructuralParameter {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                projected_qualifications: parameter.projected_qualifications.clone(),
                shape,
                placement: placement.clone(),
            },
        )
        .collect();

    Ok(PreparedUnitFunction {
        call_plan,
        scalar_parameters,
        parameters,
    })
}
