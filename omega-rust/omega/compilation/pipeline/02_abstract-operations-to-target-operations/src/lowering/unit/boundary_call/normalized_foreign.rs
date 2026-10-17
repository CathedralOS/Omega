//! Normalized foreign scalar and structural argument and result projection.

use super::super::super::control_flow::scalar_sources::{
    ScalarSources, resolved_source, runtime_indices,
};
use super::super::super::scalar_abi::fixed_native_scalar_shape;
use super::super::super::structural_layout::{
    reference_referent_projection, resolve_structural_projection_path, structural_parameter_shape,
    structural_shape,
};
use super::{
    AbstractFunction, BTreeMap, BTreeSet, BoundaryMachineId, CallSignature, LoweringError,
    NativeTarget, OperationId, PlaceId, StructuralTypeId, StructuralTypeLookup,
    StructuralTypeShape, TargetStructuralParameter, TargetUnitScalarHomeRequirement, ValueId,
    ValueLocation, ValueShape,
};
use calling_conventions::ValueClass;
use target_operations::{TargetStructuralArgument, TargetUnitScalarArgumentSource};
use terminal_psi::{StructuralAccess, StructuralPathSegment};

/// Lower source-rooted borrowed structural arguments for one evaluated
/// normalized foreign call, preserving the exact caller place, semantic field
/// path, access mode, and projected byte offset beside the evaluated plan's
/// destination placement.
///
/// The transport view retains authored order, including a dispatch-only table
/// when present; the evaluated wire plan excludes that table. Terminal retains
/// that order separately
/// from lane-local proof and custody coordinates. Private callback slots are
/// inserted by the evaluated calling plan, not by the semantic declaration.
///
/// Each admitted borrowed argument must resolve to one bounded projection
/// rooted at caller storage: a structural parameter or a dominating home.
/// Record fields and fixed or runtime-selected array
/// elements (`&record.field`, `&text[0]`), or the whole parameter when the
/// caller forwards a loan it received. The projected type equals the declared
/// parameter type, and the evaluated plan places the referent pointer as one
/// pointer-width word.
/// A borrowed dynamic descriptor — a `ByteSequence(BorrowedView)` referent —
/// instead projects either the caller's whole stored view (empty path) or a
/// stored descriptor field (field-only path), and the evaluated plan places
/// the two descriptor words by value under the selected aggregate ABI,
/// carrying the descriptor's own size and alignment. An owned argument passes the caller's
/// whole place by value — an empty path, the root type as the declared
/// parameter type, and the evaluated plan carrying the aggregate's target ABI
/// classification with the referent's exact size and alignment.
#[allow(clippy::too_many_arguments)]
pub(super) fn lower_normalized_foreign_structural_arguments(
    boundary: BoundaryMachineId,
    function: &AbstractFunction,
    scalar_sources: &ScalarSources<'_>,
    target: NativeTarget,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    structural_arguments: &[terminal_psi::StructuralArgument],
    binding: &target_operations::NormalizedForeignCallBinding,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    native_callbacks: &[target_operations::TargetNativeCallbackArgument],
    local_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    address_joins: &BTreeSet<PlaceId>,
    block_views: &BTreeSet<PlaceId>,
) -> Result<Vec<TargetStructuralArgument>, LoweringError> {
    let boundary_entry_plan = &binding.boundary_entry_plan;
    let operands = binding.operand_placements();
    // Only direct parameter destinations insert telescope slots; a field
    // destination's write flows through an already-occupied record parameter.
    let callback_ordinals = native_callbacks
        .iter()
        .filter(|callback| {
            matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            )
        })
        .map(|callback| {
            usize::try_from(callback.application.native_ordinal)
                .map(|ordinal| ordinal + binding.dispatch_operand_count())
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    let mut sorted_ordinals = callback_ordinals;
    sorted_ordinals.sort_unstable();
    if structural_arguments.len() != declaration.structural_parameters.len()
        || !declaration.has_valid_parameter_order()
        || operands.len() != declaration.parameter_order.len() + sorted_ordinals.len()
        || native_callbacks
            .iter()
            .any(|callback| callback.registrar_boundary_entry_plan != *boundary_entry_plan)
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    let pointer_size = u16::try_from(target.pointer_size)
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    let pointer_alignment = u16::try_from(target.pointer_alignment)
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Structural))
        .map(|((index, (argument, parameter)), semantic_position)| {
            let native_position = semantic_position
                + sorted_ordinals
                    .iter()
                    .filter(|ordinal| semantic_position >= **ordinal)
                    .count();
            let resolved = super::super::argument_roots::resolve(
                argument.place,
                function,
                parameters_by_place.values().copied(),
                local_homes,
                address_joins,
                block_views,
                structural_types,
                shape_cache,
                active,
            )?;
            let root = resolved.declaration;
            let source_structural_type = root.structural_type;
            let source_shape = resolved.backing_shape;
            let source_multiplicity = root.multiplicity;
            let argument_source = resolved.source;
            let permitted_access = match root.access {
                StructuralAccess::Owned => true,
                StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => {
                    argument.access != StructuralAccess::Owned || !argument.path.is_empty()
                }
                StructuralAccess::WriteOnlyBorrow => {
                    argument.access == StructuralAccess::WriteOnlyBorrow
                }
            };
            if !permitted_access
                || local_homes.get(&argument.place).is_some_and(|home| {
                    home.place() != argument.place || home.access() != StructuralAccess::Owned
                })
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            if usize::try_from(parameter.position).ok() != Some(index) {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let descriptor_formal = argument.access != StructuralAccess::Owned
                && structural_types
                    .get(&parameter.structural_type)
                    .is_some_and(|row| {
                        matches!(
                            row.shape,
                            StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                            )
                        )
                    });
            if descriptor_formal {
                let destination = operands
                    .get(native_position)
                    .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                let carrier = structural_shape(
                    parameter.structural_type,
                    structural_types,
                    shape_cache,
                    active,
                )
                .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
                let shape = structural_parameter_shape(carrier, parameter.access);
                // Fixed and dynamic byte loans use the same checked backing
                // presentation. Only a dynamic view carries a runtime extent;
                // a fixed view crosses the native ABI as one pointer.
                let bounded = matches!(
                    structural_types
                        .get(&parameter.structural_type)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    ))
                );
                if argument.access != parameter.access
                    || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || destination.shape.byte_size != carrier.byte_size
                    || destination.shape.alignment != carrier.alignment
                    || if bounded {
                        destination.shape
                            != ValueShape::integer(u32::from(pointer_size), pointer_size)
                            || calling_conventions::byte_size_at_offset_zero(&destination.locations)
                                != Some(u32::from(pointer_size))
                    } else {
                        destination.shape.class == ValueClass::BorrowedReference
                    }
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                if let Some(presentation) =
                    super::super::roster_arguments::fixed_array_view_argument(
                        argument,
                        parameter,
                        shape,
                        destination,
                        function.machine,
                        &root,
                        source_shape.byte_size,
                        argument_source.clone(),
                        structural_types,
                        shape_cache,
                        active,
                    )
                    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?
                {
                    if local_homes
                        .get(&argument.place)
                        .is_some_and(|home| home.has_claims())
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    return Ok(presentation);
                }
                if argument.access == StructuralAccess::SharedBorrow
                    && root.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && matches!(
                        root.access,
                        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                    )
                    && root.projected_qualifications.is_empty()
                    && let Some(capacity) = terminal_semantics::shared_boundary_buffer_capacity(
                        structural_types.values().copied(),
                        root.structural_type,
                        argument,
                        parameter,
                    )
                {
                    let geometry = if argument.path.is_empty() {
                        Some((0, capacity))
                    } else {
                        super::super::super::structural_layout::bounded_byte_field_geometry(
                            root.structural_type,
                            &argument.path,
                            structural_types,
                            shape_cache,
                            active,
                        )
                        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?
                    };
                    let (offset, actual_capacity) =
                        geometry.ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                    if actual_capacity != capacity
                        || u64::from(offset)
                            .checked_add(8)
                            .and_then(|end| end.checked_add(capacity))
                            .is_none_or(|end| end > u64::from(source_shape.byte_size))
                        || local_homes
                            .get(&argument.place)
                            .is_some_and(|home| home.has_claims())
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    return Ok(TargetStructuralArgument {
                        place: argument.place,
                        access: argument.access,
                        path: argument.path.clone(),
                        root_structural_type: root.structural_type,
                        structural_type: parameter.structural_type,
                        shape,
                        source_byte_offset: offset,
                        fixed_array_length: None,
                        element_stride: None,
                        indices: Vec::new(),
                        source: argument_source,
                        destination: destination.clone(),
                    });
                }
            }
            let mut index_runs = Vec::new();
            let (projected_type, projected_shape, source_byte_offset) = if parameter.access
                == StructuralAccess::Owned
            {
                // An owned argument hands the callee its subtree by value:
                // the whole caller's place at an empty path, or one member
                // carried out of a borrowed or owned root — the leaf
                // resolves through the same static projection a borrowed
                // field does, but the destination transports its bytes.
                if argument.path.is_empty() {
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
                    if argument.path.iter().any(|segment| {
                        !matches!(
                            segment,
                            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                        )
                    }) {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    resolve_structural_projection_path(
                        source_structural_type,
                        &argument.path,
                        structural_types,
                        shape_cache,
                        active,
                    )
                    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?
                }
            } else {
                // Ordinary and foreign loans use the same typed projection.
                // Each pointer crossing retains its region and selector order;
                // the checked formal supplies the selected field identity.
                let (selected, shape, offset, indices) = reference_referent_projection(
                    source_structural_type,
                    &argument.path,
                    argument.access,
                    Some(parameter.structural_type),
                    structural_types,
                )
                .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
                index_runs = indices;
                (selected, shape, offset)
            };
            // Borrowing does not transfer the owner's multiplicity. Owned
            // whole-place transport retains the exact source contract; a
            // projected member move surrenders the leaf's own custody —
            // affine, or linear when the member carries its own claim. The
            // ownership replay already placed every completed claim at this
            // owned argument; leaf-type agreement below binds the subtree to
            // the parameter's type, which stores no multiplicity of its own.
            let multiplicity_matches = if argument.access != StructuralAccess::Owned {
                parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            } else if argument.path.is_empty() {
                parameter.multiplicity == source_multiplicity
            } else {
                parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            };
            let projects_view_backing = argument.access != StructuralAccess::Owned
                && !argument.path.is_empty()
                && structural_types
                    .get(&source_structural_type)
                    .is_some_and(|row| {
                        row.shape.is_borrowed_byte_carrier()
                            || matches!(row.shape, StructuralTypeShape::ElementView { .. })
                    });
            // Qualifications and completed claims were admitted on this exact
            // operand. They do not change owned bytes or an unchanged borrow's
            // ABI; the retained caller place and path still identify custody.
            let unchanged_presentation = argument.access != StructuralAccess::Owned
                && terminal_semantics::borrowed_referent_presentation(
                    structural_types.values().copied(),
                    projected_type,
                    parameter.structural_type,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged);
            if (projected_type != parameter.structural_type && !unchanged_presentation)
                || argument.access != parameter.access
                || !multiplicity_matches
                || (!projects_view_backing
                    && !argument.path.contains(&StructuralPathSegment::Referent)
                    && projected_shape
                        .byte_size
                        .checked_add(source_byte_offset)
                        .is_none_or(|end| end > source_shape.byte_size))
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let parameter_shape = structural_parameter_shape(projected_shape, parameter.access);
            let destination = operands
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
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
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
                        let Some(placed_pointer_word) =
                            calling_conventions::byte_size_at_offset_zero(
                                destination.locations.as_slice(),
                            )
                        else {
                            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                        };
                        if destination.shape
                            != ValueShape::integer(u32::from(pointer_size), pointer_alignment)
                            || placed_pointer_word != u32::from(pointer_size)
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
                structural_type: parameter.structural_type,
                shape: parameter_shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                indices: runtime_indices(index_runs, function, scalar_sources)?,
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
    binding: &target_operations::NormalizedForeignCallBinding,
    sources: &ScalarSources<'_>,
    result_shape: Option<ValueShape>,
    native_callbacks: &[target_operations::TargetNativeCallbackArgument],
    structural_parameter_shapes: &[ValueShape],
) -> Result<Vec<target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    let boundary_entry_plan = &binding.boundary_entry_plan;
    let operands = binding.operand_placements();
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
    // Only direct parameter destinations insert telescope slots. The
    // insertion ordinals are each recorded against the pre-insertion roster.
    let mut callback_inserts = native_callbacks
        .iter()
        .filter(|callback| {
            matches!(
                callback.destination,
                calling_conventions::NativePlace::Parameter(_)
            )
        })
        .map(|callback| {
            usize::try_from(callback.application.native_ordinal)
                .map(|ordinal| (ordinal + binding.dispatch_operand_count(), callback))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    callback_inserts.sort_by_key(|(ordinal, _)| *ordinal);
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
    for (index, (ordinal, callback)) in callback_inserts.iter().enumerate() {
        let insertion = ordinal + index;
        if insertion > parameter_shapes.len() {
            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
        }
        parameter_shapes.insert(insertion, callback.application.shape);
    }
    let signature = CallSignature {
        parameters: parameter_shapes
            .into_iter()
            .skip(binding.dispatch_operand_count())
            .collect(),
        result: result_shape,
    };
    let validated = match native_callbacks.first() {
        Some(callback) => {
            for callback in native_callbacks {
                let callback_placement = usize::try_from(callback.application.native_ordinal)
                    .ok()
                    .and_then(|ordinal| boundary_entry_plan.call.parameters.get(ordinal));
                if callback.registrar_boundary_entry_plan != *boundary_entry_plan
                    || callback.application.shape != callback.application.placement.shape
                    || callback_placement != Some(&callback.application.placement)
                {
                    return Err(LoweringError::InvalidNativeCallbackArgument(
                        callback.terminal_operation,
                    ));
                }
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
        || operands.len()
            != scalar_parameter_shapes.len()
                + structural_parameter_shapes.len()
                + callback_inserts.len()
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    let inserted_ordinals = callback_inserts
        .iter()
        .map(|(ordinal, _)| *ordinal)
        .collect::<Vec<_>>();
    arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&scalar_parameter_shapes)
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
        .map(
            |(((source_value, parameter), shape), semantic_parameter_index)| {
                let parameter_index = semantic_parameter_index
                    + inserted_ordinals
                        .iter()
                        .filter(|ordinal| semantic_parameter_index >= **ordinal)
                        .count();
                let placement = operands
                    .get(parameter_index)
                    .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                // Keep entry/block coordinates as ordinary SSA sources. In
                // particular, a ranked backedge supplies a new runtime value;
                // an immediate or synthetic operation-defined home cannot
                // stand in for that parameter's identity.
                let source = resolved_source(*source_value, function, sources)
                    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
                let Some(placed_byte_size) =
                    calling_conventions::byte_size_at_offset_zero(placement.locations.as_slice())
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
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

/// Rejoin normal completion to its semantic result and physical storage.
/// The selected entry plan supplies ABI classification; storage geometry is
/// derived from the declared carrier. Translation replay independently checks
/// that classification, so equal byte size cannot substitute a calling class.
pub(super) fn lower_normalized_foreign_result(
    boundary: BoundaryMachineId,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    defining_operation: OperationId,
    result: &abstract_operations::AbstractBoundaryResult,
    boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
    structural_types: &StructuralTypeLookup<'_>,
) -> Result<target_operations::TargetBoundaryResult, LoweringError> {
    if let abstract_operations::AbstractBoundaryResult::Structural(result) = result {
        let terminal_psi::BoundaryMachineResult::Structural(expected) = &declaration.result else {
            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
        };
        if result.structural_type != expected.structural_type
            || result.multiplicity != expected.multiplicity
            || result.qualifications != expected.qualifications
            || result.projected_qualifications != expected.projected_qualifications
        {
            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
        }
        // The admitted boundary owns qualification establishment and claim
        // issuance. The ordinary aggregate home retains that exact operation
        // result beside its bytes; transport neither discards nor reissues it.
        let home = crate::lowering::control_flow::aggregate_results::home(
            defining_operation,
            result,
            structural_types,
        )?;
        let placement = boundary_entry_plan
            .call
            .result
            .as_ref()
            .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
        let shape = home.layout.shape();
        if placement.locations.is_empty()
            || placement.shape.byte_size != shape.byte_size
            || placement.shape.alignment != shape.alignment
            || placement.shape.class == ValueClass::BorrowedReference
        {
            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
        }
        return Ok(target_operations::TargetBoundaryResult::Structural(home));
    }
    let scalar = lower_normalized_foreign_scalar_result(
        boundary,
        declaration,
        defining_operation,
        result.scalar(),
        boundary_entry_plan,
    )?;
    Ok(scalar.map_or(
        target_operations::TargetBoundaryResult::Unit,
        target_operations::TargetBoundaryResult::Scalar,
    ))
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
