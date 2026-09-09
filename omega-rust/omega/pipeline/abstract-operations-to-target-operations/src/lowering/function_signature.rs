//! Shared call signature and incoming parameter placement for ordinary graphs.

use super::scalar_abi::fixed_native_integer_shape;
use super::shared::*;
use super::structural_signature::StructuralCallSignature;
use super::unit::scalar_call::KnownUnitInteger;

/// Plain primitive references need neither owned transport nor cleanup.
pub(super) fn is_primitive_write_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    !parameter.is_self
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && structural_types.get(&parameter.structural_type).is_some_and(|declaration| {
            matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                if fixed_native_integer_shape(integer).is_some())
        })
}

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

pub(super) struct PreparedFunctionSignature {
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

pub(super) fn prepare_function_signature(
    function: &AbstractFunction,
    target: NativeTarget,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<PreparedFunctionSignature, LoweringError> {
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
        match &function.result {
            AbstractFunctionResult::Structural(result) => Some(
                super::control_flow::scalar_sums::result_layout(result, structural_types)?.shape,
            ),
            _ => function.result.scalar()
                .map(|result| super::scalar::scalar_shape(result.value, result.scalar_type, false))
                .transpose()?,
        },
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

    Ok(PreparedFunctionSignature {
        call_plan,
        scalar_parameters,
        parameters,
    })
}
