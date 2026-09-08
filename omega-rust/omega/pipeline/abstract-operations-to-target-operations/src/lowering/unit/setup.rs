//! ABI planning and structural-parameter preparation for an attached Unit body.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_signature::StructuralCallSignature;
use super::scalar_call::KnownUnitInteger;

pub(super) fn integer_parameters(
    machine: MachineId,
    parameters: &[ScalarAbiValue],
) -> Result<BTreeMap<ValueId, KnownUnitInteger>, LoweringError> {
    parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter)| match parameter.scalar_type {
            ScalarType::Boolean | ScalarType::IeeeFloat(_) => None,
            ScalarType::Integer(scalar_type) => Some(
                u32::try_from(parameter_index)
                    .map(|parameter_index| {
                        (
                            parameter.value,
                            KnownUnitInteger::Parameter {
                                parameter_index,
                                scalar_type,
                            },
                        )
                    })
                    .map_err(|_| LoweringError::UnitFunctionHasScalarParameters(machine)),
            ),
        })
        .collect()
}

pub(super) struct PreparedUnitFunction {
    pub(super) call_plan: CallPlan,
    pub(super) scalar_parameters: Vec<ScalarAbiValue>,
    pub(super) parameters: Vec<TargetStructuralParameter>,
}

pub(super) fn parameters_by_place(
    parameters: &[TargetStructuralParameter],
) -> BTreeMap<PlaceId, &TargetStructuralParameter> {
    parameters
        .iter()
        .map(|parameter| (parameter.place, parameter))
        .collect()
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
            ScalarType::IeeeFloat(format) => Ok(ValueShape::float(match format {
                IeeeFloatFormat::Binary32 => 4,
                IeeeFloatFormat::Binary64 => 8,
            })),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let signature = StructuralCallSignature::derive(
        &scalar_parameter_shapes,
        &function.structural_parameters,
        None,
        structural_types,
        &mut shape_cache,
        &mut active,
    )?;
    let parameter_shapes = signature.structural_shapes();
    let call_plan = signature.plan(target)?;
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
            Ok(ScalarAbiValue {
                value: parameter.value,
                scalar_type: parameter.scalar_type,
                placement: placement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let parameters = function
        .structural_parameters
        .iter()
        .zip(parameter_shapes.iter().copied())
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
