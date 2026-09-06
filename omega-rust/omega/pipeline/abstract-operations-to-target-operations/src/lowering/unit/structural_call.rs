//! Exact projected structural arguments for an attached Unit call.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::{
    checked_align_up_u32, resolve_structural_field_path, resolve_structural_projection_path,
    structural_parameter_shape, structural_shape,
};

#[derive(Debug, Clone)]
pub(super) struct StructuralCallLocalSource {
    pub(super) structural_type: StructuralTypeId,
    pub(super) shape: ValueShape,
    pub(super) placement: ValuePlacement,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_structural_unit_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    local_sources_by_place: &BTreeMap<PlaceId, StructuralCallLocalSource>,
    scalar_values: &BTreeMap<ValueId, super::scalar_call::KnownUnitInteger>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallUnit {
        psi_operation,
        callee,
        arguments: scalar_arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("structural Unit-call lowering receives only Unit calls")
    };
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    if callee_function.result != AbstractFunctionResult::Unit {
        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
    }
    if scalar_arguments.len() != callee_function.parameters.len() {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee: *callee,
            expected: callee_function.parameters.len(),
            actual: scalar_arguments.len(),
        });
    }
    if structural_arguments.len() != callee_function.structural_parameters.len() {
        return Err(LoweringError::StructuralCallArgumentCountMismatch {
            callee: *callee,
            expected: callee_function.structural_parameters.len(),
            actual: structural_arguments.len(),
        });
    }
    let scalar_shapes = callee_function
        .parameters
        .iter()
        .map(|parameter| match parameter.scalar_type {
            ScalarType::Boolean => Ok(ValueShape::integer(1, 1)),
            ScalarType::Integer(integer_type) => fixed_native_integer_shape(integer_type)
                .ok_or(LoweringError::UnitCallTargetKindMismatch(*callee)),
            ScalarType::IeeeFloat(_) => Err(LoweringError::UnitCallTargetKindMismatch(*callee)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_shapes = callee_function
        .structural_parameters
        .iter()
        .map(|parameter| -> Result<ValueShape, LoweringError> {
            let referent = structural_shape(
                parameter.structural_type,
                structural_types,
                shape_cache,
                active,
            )?;
            Ok(structural_parameter_shape(referent, parameter.access))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(callee_shapes.iter().copied())
                .collect(),
            result: None,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let scalar_arguments = scalar_arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&scalar_shapes)
        .zip(&callee_plan.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), expected_shape), placement))| {
                let source = match parameter.scalar_type {
                    ScalarType::Boolean => {
                        if let Some((caller_parameter_index, _)) = function
                            .parameters
                            .iter()
                            .enumerate()
                            .find(|(_, caller_parameter)| {
                                caller_parameter.value == *source_value
                                    && caller_parameter.scalar_type == ScalarType::Boolean
                            })
                        {
                            TargetUnitScalarArgumentSource::Parameter {
                                parameter_index: u32::try_from(caller_parameter_index).map_err(
                                    |_| LoweringError::UnitCallTargetKindMismatch(*callee),
                                )?,
                                source_value: *source_value,
                                scalar_type: ScalarType::Boolean,
                            }
                        } else if let Some((defining_operation, value)) =
                            boolean_constants.get(source_value).copied()
                        {
                            TargetUnitScalarArgumentSource::BooleanImmediate {
                                defining_operation,
                                source_value: *source_value,
                                value,
                            }
                        } else {
                            return Err(LoweringError::UnknownValue(*source_value));
                        }
                    }
                    ScalarType::Integer(parameter_type) => {
                        let known = scalar_values
                            .get(source_value)
                            .copied()
                            .ok_or(LoweringError::UnknownValue(*source_value))?;
                        if known.scalar_type() != parameter_type {
                            return Err(LoweringError::CallArgumentTypeMismatch {
                                callee: *callee,
                                argument: *source_value,
                            });
                        }
                        known.into_target_source(*source_value)
                    }
                    ScalarType::IeeeFloat(_) => {
                        return Err(LoweringError::UnitCallTargetKindMismatch(*callee));
                    }
                };
                if source.scalar_type() != parameter.scalar_type
                    || placement.shape != *expected_shape
                {
                    return Err(LoweringError::CallArgumentTypeMismatch {
                        callee: *callee,
                        argument: *source_value,
                    });
                }
                Ok(TargetUnitScalarCallArgument {
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::UnitCallTargetKindMismatch(*callee))?,
                    source,
                    placement: placement.clone(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = structural_arguments
        .iter()
        .zip(&callee_function.structural_parameters)
        .zip(callee_shapes)
        .zip(callee_plan.parameters.iter().skip(scalar_arguments.len()))
        .map(|(((argument, callee_parameter), shape), destination)| {
            let result_source = super::projected_result::source(operations, argument.place);
            let (source_structural_type, source_shape, source_placement) =
                if let Some(source) = parameters_by_place.get(&argument.place).copied() {
                    (source.structural_type, source.shape, &source.placement)
                } else if let Some(source) = local_sources_by_place.get(&argument.place) {
                    (source.structural_type, source.shape, &source.placement)
                } else if let Some((home, placement)) = result_source {
                    (home.result.structural_type, home.layout.shape(), placement)
                } else {
                    return Err(LoweringError::UnknownStructuralArgumentPlace {
                        machine: function.machine,
                        place: argument.place,
                    });
                };
            let exact_write_only_projection = argument.access == StructuralAccess::WriteOnlyBorrow
                && callee_parameter.access == StructuralAccess::WriteOnlyBorrow
                && callee_parameter.multiplicity == StructuralMultiplicity::Unrestricted
                && parameters_by_place
                    .get(&argument.place)
                    .is_some_and(|source| {
                        source.access == StructuralAccess::WriteOnlyBorrow
                            && source.multiplicity == StructuralMultiplicity::Unrestricted
                    })
                && argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)))
                && argument
                    .path
                    .iter()
                    .skip_while(|segment| matches!(segment, StructuralPathSegment::Field(_)))
                    .all(|segment| matches!(segment, StructuralPathSegment::FixedIndex(_)));
            let (
                projected_type,
                projected_shape,
                source_byte_offset,
                fixed_array_length,
                element_stride,
            ) =
                match argument.path.as_slice() {
                    [] => (source_structural_type, source_shape, 0, None, None),
                    path if argument.access == StructuralAccess::Owned
                        && callee_parameter.multiplicity == StructuralMultiplicity::Affine
                        && (result_source.is_some()
                            || parameters_by_place.get(&argument.place).is_some_and(
                                |source| {
                                    source.access == StructuralAccess::Owned
                                        && source.multiplicity == StructuralMultiplicity::Affine
                                },
                            )) =>
                    {
                        let (selected_type, selected_shape, offset) =
                            resolve_structural_projection_path(
                                source_structural_type,
                                path,
                                structural_types,
                                shape_cache,
                                active,
                            )?;
                        let (length, stride) =
                            super::super::structural_layout::root_array_projection_metadata(
                                source_structural_type,
                                structural_types,
                                shape_cache,
                                active,
                            )?;
                        (selected_type, selected_shape, offset, length, stride)
                    }
                    path if exact_write_only_projection => {
                        let (projected_type, projected_shape, offset) =
                            resolve_structural_projection_path(
                                source_structural_type,
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
                            })?;
                        if !matches!(
                            structural_types
                                .get(&projected_type)
                                .map(|declaration| &declaration.shape),
                            Some(StructuralTypeShape::PrimitiveScalar(_))
                        ) {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        }
                        (projected_type, projected_shape, offset, None, None)
                    }
                    [StructuralPathSegment::FixedIndex(index)] => {
                        let declaration = structural_types
                            .get(&source_structural_type)
                            .copied()
                            .ok_or(LoweringError::UnknownStructuralType(source_structural_type))?;
                        let StructuralTypeShape::FixedArray { element, length } = declaration.shape
                        else {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        };
                        if *index >= length {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        }
                        let element_shape =
                            structural_shape(element, structural_types, shape_cache, active)?;
                        let stride = checked_align_up_u32(
                            u32::from(element_shape.byte_size),
                            u32::from(element_shape.alignment),
                        )
                        .ok_or(LoweringError::StructuralTypeTooLarge(
                            source_structural_type,
                        ))?;
                        let offset = u64::from(stride)
                            .checked_mul(*index)
                            .and_then(|offset| u32::try_from(offset).ok())
                            .ok_or(LoweringError::StructuralTypeTooLarge(
                                source_structural_type,
                            ))?;
                        (element, element_shape, offset, Some(length), Some(stride))
                    }
                    path @ [StructuralPathSegment::Field(_), ..]
                        if path
                            .iter()
                            .all(|segment| matches!(segment, StructuralPathSegment::Field(_))) =>
                    {
                        let (field_type, field_shape, offset) = resolve_structural_field_path(
                            source_structural_type,
                            path,
                            structural_types,
                            shape_cache,
                            active,
                        )
                        .map_err(|_| LoweringError::StructuralCallArgumentTypeMismatch {
                            callee: *callee,
                            place: argument.place,
                        })?;
                        (field_type, field_shape, offset, None, None)
                    }
                    _ => {
                        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                            callee: *callee,
                            place: argument.place,
                        });
                    }
                };
            let projected_parameter_shape =
                structural_parameter_shape(projected_shape, callee_parameter.access);
            if projected_type != callee_parameter.structural_type
                || argument.access != callee_parameter.access
                || projected_parameter_shape != shape
                || u32::from(shape.byte_size)
                    .checked_add(source_byte_offset)
                    .is_none_or(|end| end > u32::from(source_shape.byte_size))
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
                root_structural_type: source_structural_type,
                structural_type: projected_type,
                shape,
                source_byte_offset,
                fixed_array_length,
                element_stride,
                source: source_placement.clone(),
                destination: destination.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    operations.push(TargetUnitOperation::Call {
        psi_operation: *psi_operation,
        callee: *callee,
        call_plan: callee_plan,
        scalar_arguments,
        arguments,
        claim_transfers: claim_transfers.clone(),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
