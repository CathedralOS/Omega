//! Normalized foreign scalar and structural argument and result projection.

use super::super::super::control_flow::scalar_sources::{ScalarSources, resolved_source};
use super::super::super::scalar_abi::fixed_native_scalar_shape;
use super::super::super::structural_layout::{
    borrowed_view_field_offset, byte_sequence_shape, resolve_structural_field_path,
    structural_parameter_shape, structural_shape,
};
#[cfg(test)]
use super::KnownUnitInteger;
use super::{
    AbstractFunction, BTreeMap, BTreeSet, BoundaryMachineId, CallSignature, LoweringError,
    MachineId, NativeTarget, OperationId, PlaceId, StructuralAccess, StructuralPathSegment,
    StructuralTypeId, StructuralTypeLookup, StructuralTypeShape, TargetStructuralArgument,
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TargetUnitScalarHomeRequirement, ValueClass, ValueId, ValueLocation, ValueShape,
};
#[cfg(test)]
use semantic_vocabulary::BlockId;

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
/// Each admitted borrowed argument must resolve to one flat-record
/// projection: a nonempty field-only path rooted at a caller structural
/// parameter, the projected type equal to the declared parameter type, and
/// the evaluated plan placing the referent pointer as one pointer-width word.
/// A borrowed dynamic descriptor — a `ByteSequence(BorrowedView)` referent —
/// instead projects either the caller's whole stored view (empty path) or a
/// stored descriptor field (field-only path), and the evaluated plan places
/// the two-word referent behind one indirect pointer carrying the
/// descriptor's own size and alignment. An owned argument passes the caller's
/// whole place by value — an empty path, the root type as the declared
/// parameter type, and the evaluated plan carrying the aggregate's target ABI
/// classification with the referent's exact size and alignment.
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
    operations: &[TargetUnitOperation],
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
            let (source_structural_type, source_shape, argument_source) =
                if let Some(source) = parameters_by_place.get(&argument.place).copied() {
                    (
                        source.structural_type,
                        source.shape,
                        source.placement.clone().into(),
                    )
                } else if let Some((home, _placement)) =
                    super::super::projected_result::source(operations, argument.place)
                {
                    // An owned argument may transport a dominating call's exact
                    // result place: the structural home carries the producer
                    // identity the verifier's boundary-actuals arm requires.
                    let (psi_operation, _) = home
                        .operation_result()
                        .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                    (
                        home.structural_type(),
                        home.layout.shape(),
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation,
                        },
                    )
                } else {
                    return Err(LoweringError::UnknownStructuralArgumentPlace {
                        machine,
                        place: argument.place,
                    });
                };
            if usize::try_from(parameter.position).ok() != Some(index) {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let (projected_type, projected_shape, source_byte_offset) = if parameter.access
                == StructuralAccess::Owned
            {
                // An owned argument hands the callee the caller's whole
                // place by value; a field projection remains the
                // borrowed-projection form.
                if !argument.path.is_empty() {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                (
                    source_structural_type,
                    structural_shape(
                        source_structural_type,
                        structural_types,
                        shape_cache,
                        active,
                    )
                    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?,
                    0,
                )
            } else {
                // A dynamic descriptor's referent is the caller's stored
                // view itself: borrowing the whole `ByteSequence`
                // (BorrowedView) place carries an empty path, while a
                // stored descriptor field resolves through the record
                // path with the formal's declared type supplying the
                // leaf's identity.
                let descriptor_root = |structural_type: StructuralTypeId| {
                    matches!(
                        structural_types
                            .get(&structural_type)
                            .map(|declaration| &declaration.shape),
                        Some(StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView
                        ))
                    )
                };
                if argument.path.is_empty() {
                    if !descriptor_root(source_structural_type) {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    (
                        source_structural_type,
                        byte_sequence_shape(
                            terminal_psi::ByteSequenceCarrier::BorrowedView,
                            source_structural_type,
                        )
                        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?,
                        0,
                    )
                } else {
                    if argument
                        .path
                        .iter()
                        .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    match resolve_structural_field_path(
                        source_structural_type,
                        &argument.path,
                        structural_types,
                        shape_cache,
                        active,
                    ) {
                        Ok(projection) => projection,
                        Err(_) => {
                            let Some(offset) = borrowed_view_field_offset(
                                source_structural_type,
                                &argument.path,
                                structural_types,
                                shape_cache,
                                active,
                            )
                            .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?
                            else {
                                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                            };
                            if !descriptor_root(parameter.structural_type) {
                                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                            }
                            (
                                parameter.structural_type,
                                byte_sequence_shape(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                                    parameter.structural_type,
                                )
                                .map_err(|_| {
                                    LoweringError::BoundaryRealizationMismatch(boundary)
                                })?,
                                offset,
                            )
                        }
                    }
                }
            };
            // Caller storage parameters arrive with unrestricted multiplicity;
            // a dominating call's result is the consumed-once owned aggregate
            // the verifier admits under affine multiplicity.
            let expected_multiplicity = match &argument_source {
                target_operations::TargetStructuralArgumentSource::StructuralHome { .. } => {
                    terminal_psi::StructuralMultiplicity::Affine
                }
                _ => terminal_psi::StructuralMultiplicity::Unrestricted,
            };
            if projected_type != parameter.structural_type
                || argument.access != parameter.access
                || parameter.multiplicity != expected_multiplicity
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || u32::from(projected_shape.byte_size)
                    .checked_add(source_byte_offset)
                    .is_none_or(|end| end > u32::from(source_shape.byte_size))
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
                    // A borrowed scalar or record field transports one pointer
                    // word to the referent. A borrowed dynamic descriptor's
                    // referent is itself the caller's two-word view, which the
                    // plan transports by value under the target's aggregate
                    // classification — register fragments or a stack row —
                    // joined on the descriptor's exact size and alignment.
                    let descriptor_formal = matches!(
                        structural_types
                            .get(&parameter.structural_type)
                            .map(|declaration| &declaration.shape),
                        Some(StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView
                        ))
                    );
                    if descriptor_formal {
                        if destination.locations.is_empty()
                            || destination.shape.byte_size != projected_shape.byte_size
                            || destination.shape.alignment != projected_shape.alignment
                            || destination.shape.class == ValueClass::BorrowedReference
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                        }
                    } else {
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
                }
                StructuralAccess::Owned => {
                    // By-value transport carries the aggregate under its
                    // target ABI classification — SysV eightbyte classes,
                    // homogeneous-float members, or one integer word — so the
                    // destination's shape class may differ from the semantic
                    // referent class; only size, alignment, and a by-value
                    // class join the two sides.
                    if destination.locations.is_empty()
                        || destination.shape.byte_size != projected_shape.byte_size
                        || destination.shape.alignment != projected_shape.alignment
                        || destination.shape.class == ValueClass::BorrowedReference
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                }
            }
            Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: source_structural_type,
                structural_type: projected_type,
                shape: parameter_shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                source: argument_source,
                destination: destination.clone(),
            })
        })
        .collect()
}

/// Scalar foreign arguments admit every fixed-native scalar shape — Booleans,
/// fixed-width integers, and IEEE floats — and resolve their sources through
/// the same dominating-definition precedence ordinary call lanes use:
/// retained integer identities and scalar homes, then boolean and IEEE
/// immediates, block parameters, and the caller's declared parameters.
/// Aggregate ABI classification stays outside this lane; owned arguments still
/// fail closed in the structural projection.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_normalized_foreign_scalar_arguments_with_result(
    boundary: BoundaryMachineId,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    function: &AbstractFunction,
    arguments: &[ValueId],
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    sources: &ScalarSources<'_>,
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
            fixed_native_scalar_shape(*parameter)
                .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))
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
                // Keep entry/block coordinates as ordinary SSA sources. In
                // particular, a ranked backedge supplies a new runtime value;
                // an immediate or synthetic operation-defined home cannot
                // stand in for that parameter's identity.
                let source = resolved_source(*source_value, function, sources)
                    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
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
                if source.scalar_type() != *parameter
                    || source.source_value() != *source_value
                    || placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || match source {
                        TargetUnitScalarArgumentSource::Parameter { .. }
                        | TargetUnitScalarArgumentSource::BlockParameter(_)
                        | TargetUnitScalarArgumentSource::BooleanImmediate { .. }
                        | TargetUnitScalarArgumentSource::IeeeFloatImmediate { .. } => false,
                        TargetUnitScalarArgumentSource::IntegerImmediate {
                            scalar_type,
                            value,
                            ..
                        } => semantic_vocabulary::ScalarTerm::integer(scalar_type, value).is_err(),
                        TargetUnitScalarArgumentSource::Home(home) => home.shape != *shape,
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
    let scalar_homes = BTreeMap::new();
    let booleans = BTreeMap::new();
    let ieee_float_constants = BTreeMap::new();
    let scalar_block_parameters = BTreeMap::new();
    lower_normalized_foreign_scalar_arguments_with_result(
        boundary,
        declaration,
        &AbstractFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: Vec::new(),
            operations: Vec::new(),
        },
        arguments,
        boundary_entry_plan,
        &ScalarSources {
            integers: scalar_values,
            scalar_homes: &scalar_homes,
            booleans: &booleans,
            ieee_float_constants: &ieee_float_constants,
            scalar_block_parameters: &scalar_block_parameters,
        },
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
    let (declaration_result, source_value) = match (&declaration.result, result) {
        (terminal_psi::BoundaryMachineResult::Unit, None) => {
            if boundary_entry_plan.call.result.is_some() {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            return Ok(None);
        }
        (terminal_psi::BoundaryMachineResult::Scalar(declaration_result), Some(result)) => {
            if result.scalar_type != *declaration_result {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            (*declaration_result, result.value)
        }
        _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
    };
    let shape = fixed_native_scalar_shape(declaration_result)
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
    if placement.shape != shape || *byte_size != shape.byte_size {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    Ok(Some(TargetUnitScalarHomeRequirement {
        defining_operation,
        source_value,
        scalar_type: declaration_result,
        shape,
    }))
}
