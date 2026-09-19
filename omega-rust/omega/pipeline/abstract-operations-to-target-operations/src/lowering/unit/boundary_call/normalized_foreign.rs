//! Normalized foreign scalar and structural argument and result projection.

use super::super::super::scalar_abi::fixed_native_integer_shape;
use super::super::super::structural_layout::{
    resolve_structural_field_path, structural_parameter_shape,
};
use super::{
    BTreeMap, BTreeSet, BoundaryMachineId, CallSignature, KnownUnitInteger, LoweringError,
    MachineId, NativeTarget, OperationId, PlaceId, ScalarType, StructuralAccess,
    StructuralPathSegment, StructuralTypeId, StructuralTypeLookup, TargetStructuralArgument,
    TargetStructuralParameter, TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement,
    ValueId, ValueLocation, ValueShape,
};

/// Lower source-rooted borrowed structural arguments for one evaluated
/// normalized foreign call, preserving the exact caller place, semantic field
/// path, access mode, and projected byte offset beside the evaluated plan's
/// destination placement.
///
/// The evaluated `BoundaryEntryPlan` orders `call.parameters` by the authored
/// formal signature. The Terminal declaration retains that order separately
/// from lane-local proof and custody coordinates. Private callback slots are
/// inserted by the evaluated calling plan, not by the semantic declaration.
///
/// Each admitted argument must resolve to one borrowed flat-record projection:
/// a nonempty field-only path rooted at a caller structural parameter, the
/// projected type equal to the declared parameter type, and the evaluated plan
/// placing the referent pointer as one pointer-width word. By-value aggregate
/// transport needs an aggregate ABI classification this lane does not own, so
/// owned arguments fail closed until that contract lands.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_normalized_foreign_structural_arguments(
    boundary: BoundaryMachineId,
    machine: MachineId,
    target: NativeTarget,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    structural_arguments: &[terminal_psi::StructuralArgument],
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    native_callback: Option<&target_operations::TargetNativeCallbackArgument>,
) -> Result<Vec<TargetStructuralArgument>, LoweringError> {
    if structural_arguments.len() != declaration.structural_parameters.len()
        || !declaration.has_valid_parameter_order()
        || boundary_entry_plan.call.parameters.len()
            != declaration.parameter_order.len() + usize::from(native_callback.is_some())
        || native_callback
            .is_some_and(|callback| callback.registrar_boundary_entry_plan != *boundary_entry_plan)
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    let pointer_size = u16::try_from(target.pointer_size)
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    let pointer_alignment = u16::try_from(target.pointer_alignment)
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    let callback_ordinal = native_callback
        .map(|callback| usize::try_from(callback.application.native_ordinal))
        .transpose()
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
        .map(|((index, (argument, parameter)), semantic_position)| {
            let native_position = semantic_position
                + usize::from(callback_ordinal.is_some_and(|ordinal| semantic_position >= ordinal));
            let source = parameters_by_place.get(&argument.place).copied().ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine,
                    place: argument.place,
                },
            )?;
            if argument.path.is_empty()
                || argument
                    .path
                    .iter()
                    .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
                || usize::try_from(parameter.position).ok() != Some(index)
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let (projected_type, projected_shape, source_byte_offset) =
                resolve_structural_field_path(
                    source.structural_type,
                    &argument.path,
                    structural_types,
                    shape_cache,
                    active,
                )
                .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
            if projected_type != parameter.structural_type
                || argument.access != parameter.access
                || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || u32::from(projected_shape.byte_size)
                    .checked_add(source_byte_offset)
                    .is_none_or(|end| end > u32::from(source.shape.byte_size))
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let parameter_shape = structural_parameter_shape(projected_shape, parameter.access);
            let destination = boundary_entry_plan
                .call
                .parameters
                .get(native_position)
                .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
            match parameter.access {
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow => {
                    let placed_pointer_word = match destination.locations.as_slice() {
                        [
                            ValueLocation::Register {
                                value_byte_offset: 0,
                                byte_size,
                                ..
                            },
                        ]
                        | [
                            ValueLocation::Stack {
                                value_byte_offset: 0,
                                byte_size,
                                ..
                            },
                        ] => *byte_size,
                        _ => {
                            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                        }
                    };
                    if destination.shape != ValueShape::integer(pointer_size, pointer_alignment)
                        || placed_pointer_word != pointer_size
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                }
                StructuralAccess::Owned => {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
            }
            Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: source.structural_type,
                structural_type: projected_type,
                shape: parameter_shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                source: source.placement.clone().into(),
                destination: destination.clone(),
            })
        })
        .collect()
}

pub(super) fn lower_normalized_foreign_scalar_arguments_with_result(
    boundary: BoundaryMachineId,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    result_shape: Option<ValueShape>,
    native_callback: Option<&target_operations::TargetNativeCallbackArgument>,
    structural_parameter_shapes: &[ValueShape],
) -> Result<Vec<target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    if !declaration.has_valid_parameter_order()
        || structural_parameter_shapes.len() != declaration.structural_parameters.len()
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    let scalar_parameter_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            if integer_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let bytes = integer_type.bits().div_ceil(8);
            Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callback_ordinal = native_callback
        .map(|callback| usize::try_from(callback.application.native_ordinal))
        .transpose()
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    let mut scalar_shapes = scalar_parameter_shapes.iter();
    let mut structural_shapes = structural_parameter_shapes.iter();
    let mut parameter_shapes = declaration
        .parameter_order
        .iter()
        .map(|kind| {
            match kind {
                terminal_psi::BoundaryParameterKind::Scalar => scalar_shapes.next(),
                terminal_psi::BoundaryParameterKind::Structural => structural_shapes.next(),
            }
            .copied()
            .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(callback) = native_callback {
        let ordinal =
            callback_ordinal.ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
        if ordinal > parameter_shapes.len() {
            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
        }
        parameter_shapes.insert(ordinal, callback.application.shape);
    }
    let signature = CallSignature {
        parameters: parameter_shapes,
        result: result_shape,
    };
    let validated = match native_callback {
        Some(callback) => {
            let callback_placement = callback_ordinal
                .and_then(|ordinal| boundary_entry_plan.call.parameters.get(ordinal));
            if callback.registrar_boundary_entry_plan != *boundary_entry_plan
                || callback.application.shape != callback.application.placement.shape
                || callback_placement != Some(&callback.application.placement)
            {
                return Err(LoweringError::InvalidNativeCallbackArgument(
                    callback.terminal_operation,
                ));
            }
            calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                boundary_entry_plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => calling_conventions::validate_boundary_entry_plan(
            boundary_entry_plan.clone(),
            &signature,
        ),
    }
    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    if arguments.len() != declaration.scalar_parameters.len()
        || validated.plan() != boundary_entry_plan
        || boundary_entry_plan.call.parameters.len()
            != scalar_parameter_shapes.len()
                + structural_parameter_shapes.len()
                + usize::from(native_callback.is_some())
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&scalar_parameter_shapes)
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
        .map(
            |(((source_value, parameter), shape), semantic_parameter_index)| {
                let parameter_index = semantic_parameter_index
                    + usize::from(
                        callback_ordinal.is_some_and(|ordinal| semantic_parameter_index >= ordinal),
                    );
                let placement = boundary_entry_plan
                    .call
                    .parameters
                    .get(parameter_index)
                    .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                let ScalarType::Integer(integer_type) = parameter else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let Some(known) = scalar_values.get(source_value).copied() else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let source = known.into_target_source(*source_value);
                let placed_byte_size = match placement.locations.as_slice() {
                    [
                        ValueLocation::Register {
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ]
                    | [
                        ValueLocation::Stack {
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ] => *byte_size,
                    _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
                };
                if known.scalar_type() != *integer_type
                    || placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || match source {
                        TargetUnitScalarArgumentSource::Parameter { .. } => true,
                        TargetUnitScalarArgumentSource::IntegerImmediate {
                            scalar_type,
                            value,
                            ..
                        } => semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_err(),
                        TargetUnitScalarArgumentSource::BooleanImmediate { .. }
                        | TargetUnitScalarArgumentSource::IeeeFloatImmediate { .. } => true,
                        TargetUnitScalarArgumentSource::Home(home) => home.shape != *shape,
                        TargetUnitScalarArgumentSource::BlockParameter(_) => true,
                    }
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                Ok(target_operations::NormalizedForeignScalarArgument {
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?,
                    source,
                    placement: placement.clone(),
                })
            },
        )
        .collect()
}

#[cfg(test)]
pub(super) fn lower_normalized_foreign_scalar_arguments(
    boundary: BoundaryMachineId,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
) -> Result<Vec<target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    lower_normalized_foreign_scalar_arguments_with_result(
        boundary,
        declaration,
        arguments,
        boundary_entry_plan,
        scalar_values,
        None,
        None,
        &[],
    )
}

pub(super) fn lower_normalized_foreign_scalar_result(
    boundary: BoundaryMachineId,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    defining_operation: OperationId,
    result: Option<abstract_operations::AbstractResult>,
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
) -> Result<Option<TargetUnitScalarHomeRequirement>, LoweringError> {
    let (declaration_result, result) = match (&declaration.result, result) {
        (terminal_psi::BoundaryMachineResult::Unit, None) => {
            if boundary_entry_plan.call.result.is_some() {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            return Ok(None);
        }
        (
            terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(declaration_result)),
            Some(result),
        ) => {
            let ScalarType::Integer(result_type) = result.scalar_type else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            (*declaration_result, (result.value, result_type))
        }
        _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
    };
    let (source_value, result_type) = result;
    let shape = fixed_native_integer_shape(result_type)
        .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
    let Some(placement) = boundary_entry_plan.call.result.as_ref() else {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    };
    let [
        ValueLocation::Register {
            value_byte_offset: 0,
            byte_size,
            ..
        },
    ] = placement.locations.as_slice()
    else {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    };
    if declaration_result != result_type
        || placement.shape != shape
        || *byte_size != shape.byte_size
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    Ok(Some(TargetUnitScalarHomeRequirement {
        defining_operation,
        source_value,
        scalar_type: ScalarType::Integer(result_type),
        shape,
    }))
}
