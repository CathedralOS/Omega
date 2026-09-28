//! Shared call signature and incoming parameter placement for ordinary graphs.

use super::scalar_abi::fixed_native_integer_shape;
use super::structural_signature::StructuralCallSignature;
use super::unit::scalar_call::KnownUnitInteger;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::{CallPlan, ValueShape};
use semantic_vocabulary::{IeeeFloatFormat, MachineId, PlaceId, ScalarType, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    ScalarAbiValue, TargetDynamicDescriptorParameterAbi, TargetStructuralParameter,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

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
    /// The function's borrowed descriptor parameters in declared order, each
    /// bound to the two trailing pointer placements its `{instance, table}`
    /// words occupy in `call_plan`. Empty for every other signature family.
    pub(super) dynamic_parameters: Vec<TargetDynamicDescriptorParameterAbi>,
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
    structural_types: &StructuralTypeLookup<'_>,
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
    // Dynamic descriptor parameters are declared as zero-code leading
    // operations. They occupy a dense trailing lane in the evaluated plan:
    // `ordinal` is the lane index and `source_position` must trail every
    // authored non-self parameter so the two ABI words never reorder the
    // source interface.
    let declared_dynamic_parameters = function
        .operations
        .iter()
        .take_while(|operation| {
            matches!(
                operation,
                AbstractOperation::DynamicDescriptorParameter { .. }
            )
        })
        .filter_map(|operation| match operation {
            AbstractOperation::DynamicDescriptorParameter { parameter } => Some(parameter),
            _ => None,
        })
        .collect::<Vec<_>>();
    let nonself_structural_count = function
        .structural_parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    for (index, parameter) in declared_dynamic_parameters.iter().enumerate() {
        let ordinal =
            u32::try_from(index).map_err(|_| LoweringError::InvalidDynamicDescriptorParameter {
                machine: function.machine,
                ordinal: parameter.ordinal,
            })?;
        let source_position = u32::try_from(
            function.parameters.len() + nonself_structural_count + index,
        )
        .map_err(|_| LoweringError::InvalidDynamicDescriptorParameter {
            machine: function.machine,
            ordinal: parameter.ordinal,
        })?;
        if parameter.owner != function.machine
            || parameter.ordinal != ordinal
            || parameter.source_position != source_position
            || !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
        {
            return Err(LoweringError::InvalidDynamicDescriptorParameter {
                machine: function.machine,
                ordinal: parameter.ordinal,
            });
        }
    }
    let mut signature = StructuralCallSignature::derive(
        &scalar_parameter_shapes,
        &function.structural_parameters,
        match &function.result {
            AbstractFunctionResult::Structural(result) => {
                if result.multiplicity == StructuralMultiplicity::Linear
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                {
                    return Err(LoweringError::UnsupportedStructuralReturn(function.machine));
                }
                if matches!(
                    structural_types
                        .get(&result.structural_type)
                        .map(|declaration| &declaration.shape),
                    Some(terminal_psi::StructuralTypeShape::FixedArray { .. })
                ) {
                    Some(
                        super::control_flow::aggregate_results::result_home_layout(
                            result,
                            structural_types,
                        )?
                        .shape(),
                    )
                } else {
                    Some(super::structural_layout::structural_shape(
                        result.structural_type,
                        structural_types,
                        &mut shape_cache,
                        &mut active,
                    )?)
                }
            }
            _ => function
                .result
                .scalar()
                .map(|result| super::scalar::scalar_shape(result.value, result.scalar_type, false))
                .transpose()?,
        },
        structural_types,
        &mut shape_cache,
        &mut active,
    )?;
    let parameter_shapes = signature.structural_shapes().to_vec();
    if !declared_dynamic_parameters.is_empty() {
        let pointer_size = u16::try_from(target.pointer_size).map_err(|_| {
            LoweringError::InvalidDynamicDescriptorParameter {
                machine: function.machine,
                ordinal: 0,
            }
        })?;
        let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| {
            LoweringError::InvalidDynamicDescriptorParameter {
                machine: function.machine,
                ordinal: 0,
            }
        })?;
        signature.push_dynamic_parameter_words(
            declared_dynamic_parameters.len(),
            ValueShape::integer(pointer_size, pointer_alignment),
        );
    }
    let call_plan = signature.plan(target)?;
    let expected_parameter_count = function.parameters.len()
        + function.structural_parameters.len()
        + declared_dynamic_parameters.len() * 2;
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
    let descriptor_base = function.parameters.len() + function.structural_parameters.len();
    let dynamic_parameters = declared_dynamic_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TargetDynamicDescriptorParameterAbi {
            parameter: (*parameter).clone(),
            instance: call_plan.parameters[descriptor_base + index * 2].clone(),
            table: call_plan.parameters[descriptor_base + index * 2 + 1].clone(),
        })
        .collect();

    Ok(PreparedFunctionSignature {
        call_plan,
        scalar_parameters,
        parameters,
        dynamic_parameters,
    })
}
