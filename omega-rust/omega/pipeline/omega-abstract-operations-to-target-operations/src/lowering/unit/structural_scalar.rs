//! Exact projected integer store and structural-scalar call lowering for an
//! attached Unit body.

mod dynamic_arguments;

use super::super::scalar::scalar_shape;
use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::{
    direct_boolean_field_offset, direct_integer_field_offset, resolve_structural_field_path,
    structural_shape,
};
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};
pub(super) use dynamic_arguments::{
    lower_dynamic_argument_scalar_call, lower_dynamic_argument_unit_call,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_field_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        unreachable!("projected field-store lowering receives only field stores")
    };
    if !function
        .structural_parameters
        .iter()
        .any(|parameter| parameter == destination)
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || path
            .iter()
            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let target_parameter = parameters_by_place
        .get(&destination.place)
        .copied()
        .filter(|parameter| {
            parameter.structural_type == destination.structural_type
                && parameter.multiplicity == destination.multiplicity
                && parameter.access == destination.access
                && parameter.projected_qualifications == destination.projected_qualifications
        })
        .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ))?;
    if function.structural_parameters.len() != 1 {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let (source, field_byte_size) = match value.scalar_type {
        ScalarType::Integer(integer_type) => {
            let known_value = scalar_values
                .get(&value.value)
                .copied()
                .ok_or(LoweringError::UnknownValue(value.value))?;
            let exact_source = match known_value {
                KnownUnitInteger::Parameter {
                    parameter_index,
                    scalar_type,
                } => {
                    let parameter_index = usize::try_from(parameter_index).ok();
                    scalar_type == integer_type
                        && parameter_index
                            .and_then(|index| function.parameters.get(index))
                            .is_some_and(|parameter| {
                                parameter.value == value.value
                                    && parameter.scalar_type == value.scalar_type
                            })
                }
                KnownUnitInteger::Immediate { scalar_type, .. } => {
                    scalar_type == integer_type && function.parameters.is_empty()
                }
                _ => false,
            };
            if !exact_source {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            (
                known_value.into_target_source(value.value),
                integer_type.bits().div_ceil(8),
            )
        }
        ScalarType::Boolean => {
            let source = if let Some((parameter_index, _)) = function
                .parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    parameter.value == value.value && parameter.scalar_type == ScalarType::Boolean
                }) {
                TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        LoweringError::UnitFunctionHasScalarParameters(function.machine)
                    })?,
                    source_value: value.value,
                    scalar_type: ScalarType::Boolean,
                }
            } else if function.parameters.is_empty() {
                let (defining_operation, immediate) = boolean_constants
                    .get(&value.value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(value.value))?;
                TargetUnitScalarArgumentSource::BooleanImmediate {
                    defining_operation,
                    source_value: value.value,
                    value: immediate,
                }
            } else {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            };
            (source, 1)
        }
        ScalarType::IeeeFloat(_) => {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(
                function.machine,
            ));
        }
    };
    let (carrier_type, carrier_byte_offset) = if path.is_empty() {
        structural_shape(
            destination.structural_type,
            structural_types,
            shape_cache,
            active,
        )?;
        (destination.structural_type, 0)
    } else {
        let (carrier_type, _, carrier_byte_offset) = resolve_structural_field_path(
            destination.structural_type,
            path,
            structural_types,
            shape_cache,
            active,
        )?;
        (carrier_type, carrier_byte_offset)
    };
    let scalar_byte_offset = match value.scalar_type {
        ScalarType::Boolean => direct_boolean_field_offset(carrier_type, *field, structural_types)?,
        ScalarType::Integer(integer_type) => {
            direct_integer_field_offset(carrier_type, *field, integer_type, structural_types)?
        }
        ScalarType::IeeeFloat(_) => unreachable!("IEEE field stores were rejected above"),
    };
    let field_byte_offset = carrier_byte_offset
        .checked_add(scalar_byte_offset)
        .filter(|offset| {
            offset
                .checked_add(u32::from(field_byte_size))
                .is_some_and(|end| end <= u32::from(target_parameter.shape.byte_size))
        })
        .ok_or(LoweringError::StructuralTypeTooLarge(
            destination.structural_type,
        ))?;
    operations.push(TargetUnitOperation::StructuralScalarFieldStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        destination_placement: target_parameter.placement.clone(),
        field_byte_offset,
        source,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_structural_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallStructuralScalar {
        psi_operation,
        result,
        callee,
        arguments: scalar_argument_values,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("structural-scalar call lowering receives only structural scalar calls")
    };
    if structural_arguments.is_empty() {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let Some(callee_result) = callee_function.result.scalar() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    };
    if callee_result.scalar_type != result.scalar_type
        || !callee_function.published_service_ceiling.is_empty()
        || scalar_argument_values.len() != callee_function.parameters.len()
        || structural_arguments.len() != callee_function.structural_parameters.len()
    {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    let free_whole_affine = function.attachment.is_none()
        && function.structural_parameters.len() == structural_arguments.len()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && function.published_service_ceiling.is_empty()
        && function.structural_parameters.iter().all(|parameter| {
            parameter.multiplicity == StructuralMultiplicity::Affine
                && parameter.access == StructuralAccess::Owned
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        && callee_function
            .structural_parameters
            .iter()
            .all(|parameter| {
                parameter.multiplicity == StructuralMultiplicity::Affine
                    && parameter.access == StructuralAccess::Owned
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            })
        && structural_arguments.iter().all(|argument| {
            argument.path.is_empty()
                && argument.access == StructuralAccess::Owned
                && function
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == argument.place)
        })
        && structural_arguments
            .iter()
            .map(|argument| argument.place)
            .collect::<BTreeSet<_>>()
            .len()
            == structural_arguments.len();
    let attached_projection = function.attachment.is_some();
    if !free_whole_affine && !attached_projection {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let scalar_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter.scalar_type else {
                return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
            };
            fixed_native_integer_shape(integer_type)
                .ok_or(LoweringError::UnitCallTargetKindMismatch(*callee))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_shapes = callee_function
        .structural_parameters
        .iter()
        .map(|parameter| {
            structural_shape(
                parameter.structural_type,
                structural_types,
                shape_cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(callee_shapes.iter().copied())
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape) {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    let scalar_arguments = scalar_argument_values
        .iter()
        .zip(&callee_function.parameters)
        .zip(&scalar_shapes)
        .zip(&call_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), expected_shape), placement))| {
                let known = scalar_values
                    .get(source_value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*source_value))?;
                let ScalarType::Integer(parameter_type) = parameter.scalar_type else {
                    return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
                };
                if known.scalar_type() != parameter_type || placement.shape != *expected_shape {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: *callee,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::UnitCallTargetKindMismatch(*callee))?,
                    source: known.into_target_source(*source_value),
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes)
        .zip(call_plan.parameters.iter().skip(scalar_arguments.len()))
        .map(|(((argument, callee_parameter), shape), destination)| {
            let source = parameters_by_place.get(&argument.place).copied().ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                },
            )?;
            let (projected_type, projected_shape, source_byte_offset) =
                match argument.path.as_slice() {
                    [] if free_whole_affine => (source.structural_type, source.shape, 0),
                    path @ [StructuralPathSegment::Field(_), ..]
                        if attached_projection
                            && path.iter().all(|segment| {
                                matches!(segment, StructuralPathSegment::Field(_))
                            }) =>
                    {
                        resolve_structural_field_path(
                            source.structural_type,
                            path,
                            structural_types,
                            shape_cache,
                            active,
                        )
                        .map_err(|_| {
                            LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            }
                        })?
                    }
                    _ => {
                        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                            callee: *callee,
                            place: argument.place,
                        });
                    }
                };
            if projected_type != callee_parameter.structural_type
                || projected_shape != shape
                || u32::from(shape.byte_size)
                    .checked_add(source_byte_offset)
                    .is_none_or(|end| end > u32::from(source.shape.byte_size))
            {
                return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                    callee: *callee,
                    place: argument.place,
                });
            }
            Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: source.structural_type,
                structural_type: projected_type,
                shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                source: source.placement.clone(),
                destination: destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    operations.push(TargetUnitOperation::StructuralScalarCall {
        psi_operation: *psi_operation,
        result: *result,
        callee: *callee,
        call_plan,
        scalar_arguments,
        arguments,
        claim_transfers: claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
