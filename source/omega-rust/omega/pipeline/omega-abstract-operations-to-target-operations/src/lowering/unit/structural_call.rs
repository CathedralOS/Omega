//! Exact projected structural arguments for an attached Unit call.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::{
    checked_align_up_u32, resolve_structural_field_path, structural_parameter_shape,
    structural_shape,
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
        .zip(callee_plan.parameters.iter().skip(scalar_arguments.len()))
        .map(|(((argument, callee_parameter), shape), destination)| {
            let (source_structural_type, source_shape, source_placement) =
                if let Some(source) = parameters_by_place.get(&argument.place).copied() {
                    (source.structural_type, source.shape, &source.placement)
                } else if let Some(source) = local_sources_by_place.get(&argument.place) {
                    (source.structural_type, source.shape, &source.placement)
                } else {
                    return Err(LoweringError::UnknownStructuralArgumentPlace {
                        machine: function.machine,
                        place: argument.place,
                    });
                };
            let (
                projected_type,
                projected_shape,
                source_byte_offset,
                fixed_array_length,
                element_stride,
            ) =
                match argument.path.as_slice() {
                    [] => (source_structural_type, source_shape, 0, None, None),
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
                    [
                        StructuralPathSegment::FixedIndex(outer_index),
                        StructuralPathSegment::FixedIndex(inner_index),
                    ] => {
                        let declaration = structural_types
                            .get(&source_structural_type)
                            .copied()
                            .ok_or(LoweringError::UnknownStructuralType(source_structural_type))?;
                        let StructuralTypeShape::FixedArray {
                            element: inner_type,
                            length: 2,
                        } = declaration.shape
                        else {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        };
                        let inner_declaration = structural_types
                            .get(&inner_type)
                            .copied()
                            .ok_or(LoweringError::UnknownStructuralType(inner_type))?;
                        let StructuralTypeShape::FixedArray {
                            element: leaf_type,
                            length: inner_length @ (3..=15),
                        } = inner_declaration.shape
                        else {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        };
                        if *outer_index >= 2 || *inner_index >= inner_length {
                            return Err(LoweringError::StructuralCallArgumentTypeMismatch {
                                callee: *callee,
                                place: argument.place,
                            });
                        }
                        let inner_shape =
                            structural_shape(inner_type, structural_types, shape_cache, active)?;
                        let leaf_shape =
                            structural_shape(leaf_type, structural_types, shape_cache, active)?;
                        let outer_stride = checked_align_up_u32(
                            u32::from(inner_shape.byte_size),
                            u32::from(inner_shape.alignment),
                        )
                        .ok_or(LoweringError::StructuralTypeTooLarge(
                            source_structural_type,
                        ))?;
                        let inner_stride = checked_align_up_u32(
                            u32::from(leaf_shape.byte_size),
                            u32::from(leaf_shape.alignment),
                        )
                        .ok_or(LoweringError::StructuralTypeTooLarge(
                            source_structural_type,
                        ))?;
                        let offset = u64::from(outer_stride)
                            .checked_mul(*outer_index)
                            .and_then(|offset| {
                                u64::from(inner_stride)
                                    .checked_mul(*inner_index)
                                    .and_then(|inner| offset.checked_add(inner))
                            })
                            .and_then(|offset| u32::try_from(offset).ok())
                            .ok_or(LoweringError::StructuralTypeTooLarge(
                                source_structural_type,
                            ))?;
                        (leaf_type, leaf_shape, offset, Some(2), Some(outer_stride))
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
