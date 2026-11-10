/*
  Unit-operation call, boundary-settlement, normalized-foreign, scalar
  definition, and primitive/structural store lowering (`boundary_call`,
  `argument_roots`, `roster_arguments`, `scalar_definitions`,
  `write_only_primitive_store`).

  Lowers boundary and normalized foreign calls against evaluated ABI calling
  plans, tracks known integer definitions, and resolves caller argument roots
  and primitive/field stores.
*/
pub(crate) mod argument_roots {
    use crate::LoweringError;
    use crate::structural_layout::structural_shape;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractFunction;
    use abstract_operations::control_flow::address_joins::is_address_join;
    use calling_conventions::ValueShape;
    use semantic_vocabulary::PlaceId;
    use std::collections::{BTreeMap, BTreeSet};
    use target_operations::{
        TargetStructuralArgumentSource, TargetStructuralHomeOrigin,
        TargetStructuralHomeRequirement, TargetStructuralParameter,
    };
    use terminal_psi::StructuralParameterDeclaration;

    pub(crate) struct StructuralArgumentRoot {
        pub declaration: StructuralParameterDeclaration,
        pub backing_shape: ValueShape,
        pub source: TargetStructuralArgumentSource,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve<'parameter>(
        place: PlaceId,
        function: &AbstractFunction,
        parameters: impl IntoIterator<Item = &'parameter TargetStructuralParameter>,
        homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        address_joins: &BTreeSet<PlaceId>,
        block_views: &BTreeSet<PlaceId>,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<StructuralArgumentRoot, LoweringError> {
        let unknown = || LoweringError::UnknownStructuralArgumentPlace {
            machine: function.machine,
            place,
        };
        if let Some(parameter) = parameters
            .into_iter()
            .find(|parameter| parameter.place == place)
        {
            let declaration = function.structural_parameter(place).ok_or_else(unknown)?;
            return Ok(StructuralArgumentRoot {
                declaration: declaration.clone(),
                backing_shape: structural_shape(declaration.structural_type, types)?,
                source: parameter.placement.clone().into(),
            });
        }
        if let Some(home) = homes.get(&place) {
            let declaration = match &home.origin {
                TargetStructuralHomeOrigin::OperationResult { result, .. } => {
                    StructuralParameterDeclaration {
                        place: result.place,
                        position: 0,
                        structural_type: result.structural_type,
                        access: terminal_psi::StructuralAccess::Owned,
                        multiplicity: result.multiplicity,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                        additional_input_qualifications: result
                            .additional_input_qualifications
                            .clone(),
                    }
                }
                TargetStructuralHomeOrigin::BlockParameter { declaration, .. } => {
                    declaration.clone()
                }
            };
            return Ok(StructuralArgumentRoot {
                declaration,
                backing_shape: home.layout.shape(),
                source: home.argument_source(),
            });
        }
        if address_joins.contains(&place) || block_views.contains(&place) {
            let (block, declaration) = function
                .block_entries
                .iter()
                .find_map(|entry| {
                    entry
                        .structural_parameter(place)
                        .map(|parameter| (entry.block, parameter))
                })
                .ok_or_else(unknown)?;
            if address_joins.contains(&place)
                && !is_address_join(declaration, |identity| types.get(identity).copied())
            {
                return Err(unknown());
            }
            return Ok(StructuralArgumentRoot {
                declaration: declaration.clone(),
                backing_shape: structural_shape(declaration.structural_type, types)?,
                source: TargetStructuralArgumentSource::BlockParameter { block, place },
            });
        }
        Err(unknown())
    }
}
pub(crate) mod boundary_call {
    use super::super::boundary_settlements::claim_completion_only_boundary_is_exact;
    use super::super::control_flow::scalar_sources::ScalarSources;
    use super::super::scalar_abi::fixed_native_integer_shape;
    use super::super::structural_layout::structural_sum_layout;
    use super::known_integers::{KnownUnitInteger, insert_known_unit_integer, insert_scalar_home};
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use calling_conventions::{
        CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape, evaluate_call_plan,
    };
    use semantic_vocabulary::{
        BoundaryMachineId, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use target::{Architecture, NativeTarget, ObjectFormat};
    use target_operations::{
        BoundaryByteSequenceArgument, BoundaryRealization, BoundarySettlementBinding,
        TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarCallArgument,
        TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
    };
    use terminal_psi::BoundaryMachineResult;
    use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

    mod normalized_foreign {
        use super::super::super::control_flow::scalar_sources::{
            ScalarSources, resolved_source, runtime_indices,
        };
        use super::super::super::scalar_abi::fixed_native_scalar_shape;
        use super::super::super::structural_layout::{
            reference_referent_projection, resolve_structural_projection_path, structural_shape,
        };
        use super::{
            AbstractFunction, BTreeMap, BTreeSet, BoundaryMachineId, CallSignature, LoweringError,
            NativeTarget, OperationId, PlaceId, StructuralTypeLookup, StructuralTypeShape,
            TargetStructuralParameter, TargetUnitScalarHomeRequirement, ValueId, ValueShape,
        };
        use calling_conventions::ValueClass;
        use calling_conventions::{byte_size_at_offset_zero, validate_boundary_entry_plan};
        use target_operations::{TargetStructuralArgument, TargetUnitScalarArgumentSource};
        use terminal_psi::{StructuralAccess, StructuralPathSegment};

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_normalized_foreign_structural_arguments(
            boundary: BoundaryMachineId,
            function: &AbstractFunction,
            scalar_sources: &ScalarSources<'_>,
            target: NativeTarget,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            structural_arguments: &[terminal_psi::StructuralArgument],
            binding: &target_operations::NormalizedForeignCallBinding,
            structural_types: &StructuralTypeLookup<'_>,
            parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,

            native_callbacks: &[target_operations::TargetNativeCallbackArgument],
            local_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
            address_joins: &BTreeSet<PlaceId>,
            block_views: &BTreeSet<PlaceId>,
        ) -> Result<Vec<TargetStructuralArgument>, LoweringError> {
            let boundary_entry_plan = &binding.boundary_entry_plan;
            let operands = binding.operand_placements();
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
                .zip(
                    declaration
                        .parameter_positions(terminal_psi::BoundaryParameterKind::Structural),
                )
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
                    )?;
                    let root = resolved.declaration;
                    let source_structural_type = root.structural_type;
                    let source_shape = resolved.backing_shape;
                    let source_multiplicity = root.multiplicity;
                    let argument_source = resolved.source;
                    let permitted_access = match root.access {
                        StructuralAccess::Owned => true,
                        StructuralAccess::SharedBorrow => {
                            argument.access == StructuralAccess::SharedBorrow
                        }
                        StructuralAccess::MutableBorrow => {
                            argument.access != StructuralAccess::Owned || !argument.path.is_empty()
                        }
                        StructuralAccess::WriteOnlyBorrow => {
                            argument.access == StructuralAccess::WriteOnlyBorrow
                        }
                    };
                    if !permitted_access
                        || local_homes.get(&argument.place).is_some_and(|home| {
                            home.place() != argument.place
                                || home.access() != StructuralAccess::Owned
                        })
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    if usize::try_from(parameter.position).ok() != Some(index) {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    let descriptor_formal = argument.access != StructuralAccess::Owned
                        && structural_types
                            .get(parameter.structural_type)
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
                        let carrier = structural_shape(parameter.structural_type, structural_types)
                            .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
                        let shape =
                            crate::structural_signature::StructuralCallSignature::parameter_shape(
                                crate::structural_signature::ParameterLane::Foreign,
                                parameter.structural_type,
                                carrier,
                                parameter.access,
                                structural_types,
                            );
                        let bounded = matches!(
                            structural_types
                                .get(parameter.structural_type)
                                .map(|row| &row.shape),
                            Some(StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                            ))
                        );
                        if argument.access != parameter.access
                            || parameter.multiplicity
                                != terminal_psi::StructuralMultiplicity::Unrestricted
                            || destination.shape.byte_size != carrier.byte_size
                            || destination.shape.alignment != carrier.alignment
                            || if bounded {
                                destination.shape
                                    != ValueShape::integer(u32::from(pointer_size), pointer_size)
                                    || calling_conventions::byte_size_at_offset_zero(
                                        &destination.locations,
                                    ) != Some(u32::from(pointer_size))
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
                            && root.multiplicity
                                == terminal_psi::StructuralMultiplicity::Unrestricted
                            && matches!(
                                root.access,
                                StructuralAccess::Owned
                                    | terminal_psi::borrowed_access_pair_pattern!()
                            )
                            && let Some(capacity) =
                                terminal_semantics::shared_boundary_buffer_capacity(
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
                                )
                                .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?
                            };
                            let (offset, actual_capacity) = geometry
                                .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
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
                        if argument.path.is_empty() {
                            (
                                source_structural_type,
                                structural_shape(source_structural_type, structural_types)
                                    .map_err(|_| {
                                        LoweringError::BoundaryRealizationMismatch(boundary)
                                    })?,
                                0,
                            )
                        } else {
                            if argument.path.iter().any(|segment| {
                                !matches!(
                                    segment,
                                    StructuralPathSegment::Field(_)
                                        | StructuralPathSegment::FixedIndex(_)
                                )
                            }) {
                                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                            }
                            resolve_structural_projection_path(
                                source_structural_type,
                                &argument.path,
                                structural_types,
                            )
                            .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?
                        }
                    } else {
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
                            .get(source_structural_type)
                            .is_some_and(|row| {
                                row.shape.is_borrowed_byte_carrier()
                                    || matches!(row.shape, StructuralTypeShape::ElementView { .. })
                            });
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
                    let parameter_shape =
                        crate::structural_signature::StructuralCallSignature::parameter_shape(
                            crate::structural_signature::ParameterLane::Foreign,
                            parameter.structural_type,
                            projected_shape,
                            parameter.access,
                            structural_types,
                        );
                    let destination = operands
                        .get(native_position)
                        .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                    match parameter.access {
                        StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow => {
                            let descriptor_formal = matches!(
                                structural_types
                                    .get(parameter.structural_type)
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
                                    return Err(LoweringError::BoundaryRealizationMismatch(
                                        boundary,
                                    ));
                                }
                            } else {
                                let Some(access_pointer_word) =
                                    byte_size_at_offset_zero(destination.locations.as_slice())
                                else {
                                    return Err(LoweringError::BoundaryRealizationMismatch(
                                        boundary,
                                    ));
                                };
                                if destination.shape
                                    != ValueShape::integer(
                                        u32::from(pointer_size),
                                        pointer_alignment,
                                    )
                                    || access_pointer_word != u32::from(pointer_size)
                                {
                                    return Err(LoweringError::BoundaryRealizationMismatch(
                                        boundary,
                                    ));
                                }
                            }
                        }
                        StructuralAccess::Owned => {
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

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn lower_normalized_foreign_scalar_arguments_with_result(
            boundary: BoundaryMachineId,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            function: &AbstractFunction,
            arguments: &[ValueId],
            binding: &target_operations::NormalizedForeignCallBinding,
            sources: &ScalarSources<'_>,
            result_shape: Option<ValueShape>,
            native_callbacks: &[target_operations::TargetNativeCallbackArgument],
            structural_parameter_shapes: &[ValueShape],
        ) -> Result<Vec<target_operations::NormalizedForeignScalarArgument>, LoweringError>
        {
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
        None => validate_boundary_entry_plan(boundary_entry_plan.clone(), &signature),
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
                        let source = resolved_source(*source_value, function, sources)
                            .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
                        let Some(access_byte_size) = calling_conventions::byte_size_at_offset_zero(
                            placement.locations.as_slice(),
                        ) else {
                            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                        };
                        if source.scalar_type() != *parameter
                            || source.source_value() != *source_value
                            || placement.shape != *shape
                            || shape.byte_size != access_byte_size
                            || match source {
                                TargetUnitScalarArgumentSource::Parameter { .. }
                                | TargetUnitScalarArgumentSource::BlockParameter(_)
                                | TargetUnitScalarArgumentSource::BooleanImmediate { .. }
                                | TargetUnitScalarArgumentSource::IeeeFloatImmediate { .. } => {
                                    false
                                }
                                TargetUnitScalarArgumentSource::IntegerImmediate {
                                    scalar_type,
                                    value,
                                    ..
                                } => semantic_vocabulary::ScalarTerm::integer(scalar_type, value)
                                    .is_err(),
                                TargetUnitScalarArgumentSource::Home(home) => home.shape != *shape,
                            }
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                        }
                        Ok(target_operations::NormalizedForeignScalarArgument {
                            parameter_index: u32::try_from(parameter_index).map_err(|_| {
                                LoweringError::BoundaryRealizationMismatch(boundary)
                            })?,
                            source,
                            placement: placement.clone(),
                        })
                    },
                )
                .collect()
        }

        pub(crate) fn lower_normalized_foreign_result(
            boundary: BoundaryMachineId,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            defining_operation: OperationId,
            result: &abstract_operations::AbstractBoundaryResult,
            boundary_entry_plan: &calling_conventions::BoundaryEntryPlan,
            structural_types: &StructuralTypeLookup<'_>,
        ) -> Result<target_operations::TargetBoundaryResult, LoweringError> {
            if let abstract_operations::AbstractBoundaryResult::Structural(result) = result {
                let terminal_psi::BoundaryMachineResult::Structural(expected) = &declaration.result
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                if result.structural_type != expected.structural_type
                    || result.multiplicity != expected.multiplicity
                    || result.qualifications != expected.qualifications
                    || result.projected_qualifications != expected.projected_qualifications
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                let home = crate::control_flow::aggregate_results::home(
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

        pub(crate) fn lower_normalized_foreign_scalar_result(
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
                    if result.scalar_type != declaration_result.scalar_type {
                        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                    }
                    (declaration_result.scalar_type, result.value)
                }
                _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
            };
            let shape = fixed_native_scalar_shape(declaration_result)
                .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
            let Some(placement) = boundary_entry_plan.call.result.as_ref() else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            if placement.shape != shape || !placement.is_complete_scalar_result() {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            Ok(Some(TargetUnitScalarHomeRequirement {
                defining_operation,
                source_value,
                scalar_type: declaration_result,
                shape,
            }))
        }
    }

    use normalized_foreign::{
        lower_normalized_foreign_result, lower_normalized_foreign_scalar_arguments_with_result,
    };

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_boundary_call(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        target: NativeTarget,
        structural_types: &StructuralTypeLookup<'_>,
        boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
        settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
        native_callbacks: &BTreeMap<
            OperationId,
            Vec<target_operations::TargetNativeCallbackArgument>,
        >,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
        local_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
        address_joins: &BTreeSet<PlaceId>,
        block_views: &BTreeSet<PlaceId>,

        established_byte_sequences: &BTreeMap<
            PlaceId,
            (OperationId, StructuralTypeDeclaration, Vec<u8>),
        >,
        scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
        scalar_homes: &mut crate::control_flow::ScalarHomes,
        booleans: &BTreeMap<ValueId, (OperationId, bool)>,
        ieee_float_constants: &BTreeMap<
            ValueId,
            (OperationId, semantic_vocabulary::IeeeFloatValue),
        >,
        scalar_block_parameters: &BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        match operation {
            AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } => {
                let native_callbacks_slice = native_callbacks
                    .get(psi_operation)
                    .map_or(&[][..], Vec::as_slice);
                let binding = settlements
                    .get(boundary)
                    .cloned()
                    .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
                let declaration = boundary_machines
                    .get(boundary)
                    .copied()
                    .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
                if let target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                    foreign,
                ) = &binding.realization
                {
                    let target_result = lower_normalized_foreign_result(
                        *boundary,
                        declaration,
                        *psi_operation,
                        result,
                        &foreign.boundary_entry_plan,
                        structural_types,
                    )?;
                    let structural_arguments =
                        normalized_foreign::lower_normalized_foreign_structural_arguments(
                            *boundary,
                            function,
                            &ScalarSources {
                                integers: scalar_values,
                                scalar_homes: &*scalar_homes,
                                booleans,
                                ieee_float_constants,
                                scalar_block_parameters,
                            },
                            target,
                            declaration,
                            structural_arguments,
                            foreign,
                            structural_types,
                            parameters_by_place,
                            native_callbacks_slice,
                            local_homes,
                            address_joins,
                            block_views,
                        )?;
                    let structural_parameter_shapes = structural_arguments
                        .iter()
                        .map(|argument| argument.destination.shape)
                        .collect::<Vec<_>>();
                    let scalar_arguments = lower_normalized_foreign_scalar_arguments_with_result(
                        *boundary,
                        declaration,
                        function,
                        arguments,
                        foreign,
                        &ScalarSources {
                            integers: scalar_values,
                            scalar_homes: &*scalar_homes,
                            booleans,
                            ieee_float_constants,
                            scalar_block_parameters,
                        },
                        foreign
                            .boundary_entry_plan
                            .call
                            .result
                            .as_ref()
                            .map(|result| result.shape),
                        native_callbacks_slice,
                        &structural_parameter_shapes,
                    )?;
                    if arguments.len() != declaration.scalar_parameters.len()
                        || !foreign.has_valid_entry_mechanism()
                        || foreign.native_target() != Some(target)
                        || native_callbacks_slice.iter().any(|callback| {
                            callback.terminal_operation != *psi_operation
                                || callback.registrar_boundary_entry_plan
                                    != foreign.boundary_entry_plan
                        })
                    {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    let target_operations::BoundaryExecutionBinding::AdmittedProvider(
                        provider_execution,
                    ) = binding.execution
                    else {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    };
                    if let Some(home) = target_result.scalar_home().copied() {
                        insert_scalar_home(scalar_values, scalar_homes, home)?;
                    }
                    let catalog = structural_types.catalog();
                    let descriptor_input = target_operations::descriptor_input_guard_for_call(
                        foreign,
                        catalog,
                        &scalar_arguments,
                        &structural_arguments,
                        &target_result,
                        |buffer| crate::borrowed_storage::capture_type(buffer, structural_types),
                    )
                    .map_err(|reason| LoweringError::DescriptorInputGuard {
                        machine: function.machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                        reason,
                    })?;
                    operations.push(TargetUnitOperation::NormalizedForeignCall {
                        psi_operation: *psi_operation,
                        boundary: *boundary,
                        provider_execution,
                        binding: foreign.clone(),
                        scalar_arguments,
                        structural_arguments,
                        result: target_result,
                        descriptor_input,
                    });
                    provenance.operations.push(*psi_operation);
                    return Ok(());
                }
                let target_operations::BoundarySettlementRealization::Builtin(realization) =
                    binding.realization
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                };
                let target_result = match result {
                    abstract_operations::AbstractBoundaryResult::Unit => {
                        if !declaration.result.is_unit() {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        target_operations::TargetBoundaryResult::Unit
                    }
                    abstract_operations::AbstractBoundaryResult::Structural(result) => {
                        let terminal_psi::BoundaryMachineResult::Structural(expected) =
                            &declaration.result
                        else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        if result.structural_type != expected.structural_type
                            || result.multiplicity != expected.multiplicity
                            || result.qualifications != expected.qualifications
                            || result.projected_qualifications != expected.projected_qualifications
                            || !result.projected_qualifications.is_empty()
                            || !result.claims.is_empty()
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        let layout =
                            structural_sum_layout(result.structural_type, structural_types)?;
                        target_operations::TargetBoundaryResult::Structural(
                            target_operations::TargetStructuralHomeRequirement {
                                origin:
                                    target_operations::TargetStructuralHomeOrigin::OperationResult {
                                        operation: *psi_operation,
                                        result: result.clone(),
                                    },
                                layout: target_operations::TargetStructuralHomeLayout::Sum(layout),
                            },
                        )
                    }
                    abstract_operations::AbstractBoundaryResult::Scalar(result) => {
                        let BoundaryRealization::DirectPortReadU8(_) = realization else {
                            return Err(
                            LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                                machine: function.machine,
                                operation: *psi_operation,
                                boundary: *boundary,
                            },
                        );
                        };
                        let u8_type =
                            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is a valid type");
                        if result.scalar_type != ScalarType::Integer(u8_type)
                            || declaration.result
                                != BoundaryMachineResult::Scalar(
                                    ScalarType::Integer(u8_type).into(),
                                )
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        target_operations::TargetBoundaryResult::Scalar(
                            TargetUnitScalarHomeRequirement {
                                defining_operation: *psi_operation,
                                source_value: result.value,
                                scalar_type: ScalarType::Integer(u8_type),
                                shape: ValueShape::integer(1, 1),
                            },
                        )
                    }
                };
                let scalar_arguments = Vec::new();
                let mut runtime_scalar_arguments = Vec::new();
                let mut byte_sequence_arguments = Vec::new();
                if !matches!(realization, BoundaryRealization::HostedReadByte(_))
                    && !matches!(
                        (realization, &target_result),
                        (
                            BoundaryRealization::DirectPortReadU8(_),
                            target_operations::TargetBoundaryResult::Scalar(_)
                        )
                    )
                    && !matches!(
                        &target_result,
                        target_operations::TargetBoundaryResult::Unit
                    )
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                }
                match realization {
                    BoundaryRealization::MetadataOnlyPort(realization) => {
                        if !arguments.is_empty()
                            || !matches!(
                                operations.last(),
                                Some(TargetUnitOperation::PortWrite {
                                    psi_operation,
                                    service,
                                    port,
                                    value,
                                }) if *psi_operation == realization.effect_operation
                                    && *service == realization.service
                                    && *port == realization.port
                                    && *value == realization.value
                            )
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        for argument in structural_arguments {
                            if !parameters_by_place.contains_key(&argument.place) {
                                return Err(LoweringError::UnknownStructuralArgumentPlace {
                                    machine: function.machine,
                                    place: argument.place,
                                });
                            }
                        }
                    }
                    BoundaryRealization::ClaimCompletionOnly(_) => {
                        if !claim_completion_only_boundary_is_exact(
                            function,
                            declaration,
                            arguments,
                            structural_arguments,
                            completion_claim_sources,
                            completion_receipts,
                            parameters_by_place,
                        ) {
                            return Err(LoweringError::InvalidClaimCompletionOnlyShape {
                                machine: function.machine,
                                operation: *psi_operation,
                                boundary: *boundary,
                            });
                        }
                    }
                    BoundaryRealization::LinuxWriteLine(_) => {
                        if target.object_format != ObjectFormat::Elf
                            || !matches!(
                                target.architecture,
                                Architecture::X86_64 | Architecture::Aarch64
                            )
                            || !arguments.is_empty()
                            || !declaration.result.is_unit()
                            || !declaration.scalar_parameters.is_empty()
                            || structural_arguments.len() != 1
                            || declaration.structural_parameters.len() != 1
                        {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        }
                        let argument = &structural_arguments[0];
                        let parameter = &declaration.structural_parameters[0];
                        let Some((literal_operation, structural_type, bytes)) =
                            established_byte_sequences.get(&argument.place)
                        else {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        };
                        if !argument.path.is_empty()
                            || parameter.position != 0
                            || parameter.structural_type != structural_type.id
                            || parameter.multiplicity
                                != terminal_psi::StructuralMultiplicity::Unrestricted
                            || !parameter.qualifications.is_empty()
                        {
                            return Err(LoweringError::LinuxWriteLineUnsupportedOrInvalid {
                                machine: function.machine,
                                boundary: *boundary,
                                target,
                            });
                        }
                        byte_sequence_arguments.push(BoundaryByteSequenceArgument {
                            argument: argument.clone(),
                            literal_operation: *literal_operation,
                            structural_type: structural_type.clone(),
                            bytes: bytes.clone(),
                        });
                    }
                    BoundaryRealization::HostedReadByte(_) => {
                        let target_operations::TargetBoundaryResult::Structural(home) =
                            &target_result
                        else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        let Some(result_declaration) = structural_types.get(home.structural_type())
                        else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        let StructuralTypeShape::Sum { cases } = &result_declaration.shape else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        let [empty, byte] = cases.as_slice() else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        let [field] = byte.fields.as_slice() else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        let valid_payload = empty.fields.is_empty()
                            && !field.relevance.is_erased()
                            && match field.field_type {
                                StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                                    !integer.is_address()
                                        && integer.sign() == IntegerSign::Signed
                                        && integer.bits() == 32
                                }
                                StructuralFieldType::BoundedInteger(bounds) => {
                                    let integer = bounds.integer_type();
                                    !integer.is_address()
                                        && integer.sign() == IntegerSign::Signed
                                        && integer.bits() == 32
                                        && bounds
                                            .contains(semantic_vocabulary::IntegerValue::Signed(0))
                                        && bounds.contains(
                                            semantic_vocabulary::IntegerValue::Signed(255),
                                        )
                                }
                                _ => false,
                            };
                        if !target_operations::HostedReadByteRealization::supports_target(target)
                            || !valid_payload
                            || !arguments.is_empty()
                            || !structural_arguments.is_empty()
                            || !declaration.scalar_parameters.is_empty()
                            || !declaration.structural_parameters.is_empty()
                            || !completion_claim_sources.is_empty()
                            || !completion_receipts.is_empty()
                            || !matches!(
                                &target_result,
                                target_operations::TargetBoundaryResult::Structural(home)
                                    if home.layout.sum().is_some_and(|layout| layout.tag_byte_offset == 0
                                        && layout.tag_shape == ValueShape::integer(4, 4))
                            )
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                    }
                    BoundaryRealization::HostedWriteByteI32(_) => {
                        let i32_type =
                            IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
                        let [source_value] = arguments.as_slice() else {
                            return Err(LoweringError::InvalidHostedExitProcessShape(
                                function.machine,
                            ));
                        };
                        let Some(known) = scalar_values.get(source_value).copied() else {
                            return Err(LoweringError::UnknownValue(*source_value));
                        };
                        let shape = fixed_native_integer_shape(i32_type).ok_or(
                            LoweringError::InvalidHostedExitProcessShape(function.machine),
                        )?;
                        let call_plan = evaluate_call_plan(
                            CallingPolicy::native_for_target(target),
                            &CallSignature {
                                parameters: vec![shape],
                                result: None,
                            },
                        )
                        .map_err(LoweringError::AbiPlan)?;
                        let [placement] = call_plan.parameters.as_slice() else {
                            return Err(LoweringError::InvalidHostedExitProcessShape(
                                function.machine,
                            ));
                        };
                        let supports_target =
                            target_operations::HostedWriteByteI32Realization::supports_target(
                                target,
                            );
                        if !supports_target
                            || declaration.scalar_parameters.as_slice()
                                != [ScalarType::Integer(i32_type)]
                            || !declaration.structural_parameters.is_empty()
                            || !declaration.result.is_unit()
                            || known.scalar_type() != i32_type
                            || !structural_arguments.is_empty()
                            || placement.shape != shape
                        {
                            return Err(LoweringError::InvalidHostedExitProcessShape(
                                function.machine,
                            ));
                        }
                        runtime_scalar_arguments.push(TargetUnitScalarCallArgument {
                            parameter_index: 0,
                            source: known.into_target_source(*source_value),
                            placement: placement.clone(),
                        });
                    }
                    BoundaryRealization::HostedExitProcessI32(_) => {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    BoundaryRealization::ProgramImageRetainedSuffixStart
                    | BoundaryRealization::NearestIeeeFloatArithmetic(_)
                    | BoundaryRealization::AccessorScalarAccess(_)
                    | BoundaryRealization::EmptyRunRelease
                    | BoundaryRealization::StaticInterrupt { .. }
                    | BoundaryRealization::AccessLifecycle { .. } => {
                        return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                    }
                    BoundaryRealization::DirectPortReadU8(_) => {
                        let call_plan = evaluate_call_plan(
                            CallingPolicy::native_for_target(target),
                            &CallSignature {
                                parameters: Vec::new(),
                                result: Some(ValueShape::integer(1, 1)),
                            },
                        )
                        .map_err(LoweringError::AbiPlan)?;
                        if target.architecture != Architecture::X86_64
                            || call_plan.result
                                != Some(ValuePlacement {
                                    shape: ValueShape::integer(1, 1),
                                    locations: vec![ValueLocation::Register {
                                        register: calling_conventions::MachineRegister::X86Rax,
                                        value_byte_offset: 0,
                                        byte_size: 1,
                                    }],
                                })
                            || !arguments.is_empty()
                            || !declaration.scalar_parameters.is_empty()
                            || !completion_claim_sources.is_empty()
                            || !completion_receipts.is_empty()
                            || !structural_arguments.iter().all(|argument| {
                                argument.path.is_empty()
                                    && parameters_by_place.contains_key(&argument.place)
                            })
                        {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        }
                        let target_operations::TargetBoundaryResult::Scalar(home) = &target_result
                        else {
                            return Err(LoweringError::BoundaryRealizationMismatch(*boundary));
                        };
                        insert_known_unit_integer(
                            scalar_values,
                            home.source_value,
                            KnownUnitInteger::Home(*home),
                        )?;
                    }
                }
                operations.push(TargetUnitOperation::BoundarySettlement {
                    psi_operation: *psi_operation,
                    boundary: *boundary,
                    result: target_result,
                    execution: binding.execution,
                    realization,
                    scalar_arguments,
                    runtime_scalar_arguments,
                    arguments: structural_arguments.clone(),
                    byte_sequence_arguments,
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                });
                provenance.operations.push(*psi_operation);
            }
            _ => unreachable!("boundary-call routing admits only boundary calls"),
        }
        Ok(())
    }
}
pub(crate) mod known_integers {
    use crate::LoweringError;
    use semantic_vocabulary::{
        BlockId, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };
    use std::collections::BTreeMap;
    use target_operations::{TargetUnitScalarArgumentSource, TargetUnitScalarHomeRequirement};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum KnownUnitInteger {
        Parameter {
            parameter_index: u32,
            scalar_type: IntegerType,
        },
        Immediate {
            defining_operation: OperationId,
            scalar_type: IntegerType,
            value: IntegerValue,
        },
        Home(TargetUnitScalarHomeRequirement),
        BlockParameter {
            block: BlockId,
            value: ValueId,
            scalar_type: IntegerType,
        },
    }

    impl KnownUnitInteger {
        pub(crate) fn scalar_type(self) -> IntegerType {
            match self {
                Self::Parameter { scalar_type, .. } => scalar_type,
                Self::Immediate { scalar_type, .. } => scalar_type,
                Self::Home(home) => match home.scalar_type {
                    ScalarType::Integer(integer) => integer,
                    _ => unreachable!("known Unit integer home retains an integer type"),
                },
                Self::BlockParameter { scalar_type, .. } => scalar_type,
            }
        }

        pub(crate) const fn into_target_source(
            self,
            source_value: ValueId,
        ) -> TargetUnitScalarArgumentSource {
            match self {
                Self::Parameter {
                    parameter_index,
                    scalar_type,
                } => TargetUnitScalarArgumentSource::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type: ScalarType::Integer(scalar_type),
                },
                Self::Immediate {
                    defining_operation,
                    scalar_type,
                    value,
                } => TargetUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                Self::Home(home) => TargetUnitScalarArgumentSource::Home(home),
                Self::BlockParameter {
                    block,
                    value,
                    scalar_type,
                } => TargetUnitScalarArgumentSource::BlockParameter(
                    target_operations::TargetScalarBlockValue {
                        block,
                        value,
                        scalar_type: ScalarType::Integer(scalar_type),
                    },
                ),
            }
        }
    }

    pub(crate) fn insert_known_unit_integer(
        values: &mut BTreeMap<ValueId, KnownUnitInteger>,
        value: ValueId,
        known: KnownUnitInteger,
    ) -> Result<(), LoweringError> {
        if values.insert(value, known).is_some() {
            return Err(LoweringError::DuplicateValue(value));
        }
        Ok(())
    }

    pub(crate) fn insert_scalar_home(
        integers: &mut BTreeMap<ValueId, KnownUnitInteger>,
        scalar_homes: &mut crate::control_flow::ScalarHomes,
        home: TargetUnitScalarHomeRequirement,
    ) -> Result<(), LoweringError> {
        match home.scalar_type {
            ScalarType::Integer(_) => {
                insert_known_unit_integer(integers, home.source_value, KnownUnitInteger::Home(home))
            }
            ScalarType::Boolean | ScalarType::IeeeFloat(_) => {
                if std::sync::Arc::make_mut(scalar_homes)
                    .insert(home.source_value, home)
                    .is_some()
                {
                    return Err(LoweringError::DuplicateValue(home.source_value));
                }
                Ok(())
            }
        }
    }
}
mod owned_store_home {
    use semantic_vocabulary::{BlockId, PlaceId};
    use std::collections::BTreeMap;
    use target_operations::{TargetStructuralHomeOrigin, TargetStructuralHomeRequirement};
    use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

    pub(crate) fn block_home<'homes>(
        homes: &'homes BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        block: BlockId,
        parameter: &StructuralParameterDeclaration,
    ) -> Option<&'homes TargetStructuralHomeRequirement> {
        let home = homes.get(&parameter.place)?;
        (parameter.access == StructuralAccess::Owned
            && matches!(&home.origin,
            TargetStructuralHomeOrigin::BlockParameter { block: owner, declaration }
                if *owner == block && declaration == parameter))
        .then_some(home)
    }
}
mod projected_result {
    use calling_conventions::{ValueClass, ValueLocation, ValuePlacement};
    use semantic_vocabulary::PlaceId;
    use target_operations::TargetUnitOperation;
    use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
    use terminal_psi::StructuralMultiplicity;

    pub(crate) fn has_store_fragments(placement: &ValuePlacement) -> bool {
        placement.shape.class == ValueClass::Integer
            && placement.shape.byte_size != 0
            && placement
                .locations
                .iter()
                .try_fold(0_u32, |cursor, location| {
                    let ValueLocation::Register {
                        value_byte_offset,
                        byte_size,
                        ..
                    } = location
                    else {
                        return None;
                    };
                    if *value_byte_offset != cursor || !matches!(byte_size, 1..=8) {
                        return None;
                    }
                    cursor.checked_add(*byte_size)
                })
                == Some(placement.shape.byte_size)
    }

    pub(crate) fn source(
        operations: &[TargetUnitOperation],
        place: PlaceId,
    ) -> Option<(&TargetStructuralHomeRequirement, &ValuePlacement)> {
        let mut sources = operations.iter().filter_map(|operation| {
            let TargetUnitOperation::Call {
                psi_operation,
                result:
                    target_operations::TargetCallResult::Structural {
                        result,
                        result_home: Some(home),
                        ..
                    },
                call_plan,
                ..
            } = operation
            else {
                return None;
            };
            let TargetStructuralHomeLayout::Aggregate(shape) = home.layout else {
                return None;
            };
            let placement = call_plan.result.as_ref()?;
            (result.place == place
                && home.operation_result() == Some((*psi_operation, result))
                && shape == placement.shape
                && has_store_fragments(placement)
                && result.multiplicity == StructuralMultiplicity::Affine
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty())
            .then_some((home, placement))
        });
        let result = sources.next()?;
        sources.next().is_none().then_some(result)
    }
}
pub(crate) mod roster_arguments {
    use super::super::scalar::byte_views::{block_source, is_immutable_byte_carrier_parameter};
    use super::super::structural_layout::{
        checked_align_up_u32, resolve_structural_projection_path, structural_shape,
    };
    use crate::LoweringError;
    use crate::control_flow::scalar_sources::{ScalarSources, runtime_indices};
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractFunction;
    use calling_conventions::{ValuePlacement, ValueShape};
    use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
    use std::collections::BTreeMap;
    use target_operations::{
        TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeRequirement,
        TargetStructuralParameter, TargetUnitOperation,
    };
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

    fn argument_type_mismatch(callee: MachineId, place: PlaceId) -> LoweringError {
        LoweringError::StructuralCallArgumentTypeMismatch { callee, place }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_structural_argument(
        argument: &terminal_psi::StructuralArgument,
        callee_parameter: &terminal_psi::StructuralParameterDeclaration,
        shape: ValueShape,
        destination: &ValuePlacement,
        callee: MachineId,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
        local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        established_views: &BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
        operations: &[TargetUnitOperation],
        scalar_sources: &ScalarSources<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        let owner_home = owned_owner_home(
            argument,
            callee_parameter,
            function,
            local_homes,
            structural_types,
        );
        if let Some(producer) = owner_home {
            let home = local_homes.get(&argument.place).ok_or(
                LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                },
            )?;
            let owner = home.layout.shape();
            if shape
                != crate::structural_signature::StructuralCallSignature::parameter_shape(
                    crate::structural_signature::ParameterLane::Internal,
                    callee_parameter.structural_type,
                    owner,
                    argument.access,
                    structural_types,
                )
            {
                return Err(argument_type_mismatch(callee, argument.place));
            }
            return Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: Vec::new(),
                root_structural_type: callee_parameter.structural_type,
                structural_type: callee_parameter.structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                indices: Vec::new(),
                source: TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: producer,
                },
                destination: destination.clone(),
            });
        }
        if let Some((source, structural_type)) = established_views
            .get(&argument.place)
            .map(|(producer, structural_type)| {
                (
                    target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                        psi_operation: *producer,
                    },
                    *structural_type,
                )
            })
            .or_else(|| block_source(function, argument.place, structural_types))
        {
            if !argument.path.is_empty()
                || argument.access != StructuralAccess::SharedBorrow
                || !is_immutable_byte_carrier_parameter(callee_parameter, structural_types)
                || structural_type != callee_parameter.structural_type
                || shape != ValueShape::borrowed_reference(16, 8)
            {
                return Err(argument_type_mismatch(callee, argument.place));
            }
            return Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: Vec::new(),
                root_structural_type: structural_type,
                structural_type,
                shape,
                source_byte_offset: 0,
                fixed_array_length: None,
                element_stride: None,
                indices: Vec::new(),
                source,
                destination: destination.clone(),
            });
        }
        let result_source = super::projected_result::source(operations, argument.place);
        let (source_structural_type, source_shape, source_placement) =
            if let Some(source) = parameters_by_place.get(&argument.place).copied() {
                (source.structural_type, source.shape, &source.placement)
            } else if let Some((home, placement)) = result_source {
                (home.structural_type(), home.layout.shape(), placement)
            } else if let Some(home) = local_homes.get(&argument.place) {
                return local_array_loan(
                    argument,
                    callee_parameter,
                    shape,
                    destination,
                    callee,
                    home,
                    structural_types,
                )?
                .ok_or(LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                });
            } else {
                return Err(LoweringError::UnknownStructuralArgumentPlace {
                    machine: function.machine,
                    place: argument.place,
                });
            };
        if let Some(root) = function.structural_parameter(argument.place)
            && let Some(loan) = fixed_array_view_argument(
                argument,
                callee_parameter,
                shape,
                destination,
                callee,
                root,
                source_shape.byte_size,
                source_placement.clone().into(),
                structural_types,
            )?
        {
            return Ok(loan);
        }
        let (selected, selected_shape, source_byte_offset, indices) =
            super::super::structural_layout::runtime_storage_projection(
                source_structural_type,
                &argument.path,
                structural_types,
            )?;
        let indices = runtime_indices(indices, function, scalar_sources)?;
        let mut byte_field_capacity = None;
        let (projected_type, projected_shape) = match selected {
            super::super::structural_layout::ProjectedStorage::Field(field)
                if field.field_type.byte_sequence_carrier().is_some() =>
            {
                match field.field_type.byte_sequence_carrier() {
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) => {
                        byte_field_capacity = Some(capacity);
                    }
                    Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    }) if callee_parameter.access == StructuralAccess::SharedBorrow
                        && structural_types
                            .get(callee_parameter.structural_type)
                            .is_some_and(|row| row.shape.is_shared_readable_byte_view()) => {}
                    _ => return Err(argument_type_mismatch(callee, argument.place)),
                }
                (
                    callee_parameter.structural_type,
                    structural_shape(callee_parameter.structural_type, structural_types)?,
                )
            }
            selected => (
                selected.declaration(
                    source_structural_type,
                    None,
                    structural_types,
                    terminal_psi::StructuralFieldType::canonical_leaf_type,
                )?,
                selected_shape,
            ),
        };
        let projected_argument = move || TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: source_structural_type,
            structural_type: callee_parameter.structural_type,
            shape,
            source_byte_offset,
            fixed_array_length: None,
            element_stride: None,
            indices,
            source: source_placement.clone().into(),
            destination: destination.clone(),
        };
        if let Some(capacity) = byte_field_capacity {
            let source_is_subloan =
                parameters_by_place
                    .get(&argument.place)
                    .is_some_and(|source| {
                        source.multiplicity == StructuralMultiplicity::Unrestricted
                            && source.projected_qualifications.is_empty()
                            && match argument.access {
                                StructuralAccess::SharedBorrow => {
                                    matches!(
                                        source.access,
                                        terminal_psi::reading_borrow_pair_pattern!()
                                    )
                                }
                                StructuralAccess::MutableBorrow => {
                                    source.access == StructuralAccess::MutableBorrow
                                }
                                StructuralAccess::WriteOnlyBorrow => matches!(
                                    source.access,
                                    terminal_psi::exclusive_borrow_pair_pattern!()
                                ),
                                _ => false,
                            }
                    });
            let presented_is_byte_view = matches!(
                structural_types
                    .get(callee_parameter.structural_type)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                ))
            );
            let presented_is_owner = structural_types
                .get(callee_parameter.structural_type)
                .is_some_and(|declaration| {
                    declaration.shape
                        == StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                        )
                });
            let expected_shape = if presented_is_owner {
                crate::structural_signature::StructuralCallSignature::parameter_shape(
                    crate::structural_signature::ParameterLane::Internal,
                    callee_parameter.structural_type,
                    projected_shape,
                    callee_parameter.access,
                    structural_types,
                )
            } else {
                ValueShape::borrowed_reference(16, 8)
            };
            if !(presented_is_byte_view || presented_is_owner)
                || argument.access != callee_parameter.access
                || !matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || callee_parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || (!presented_is_owner && !callee_parameter.qualifications.is_empty())
                || !callee_parameter.projected_qualifications.is_empty()
                || !source_is_subloan
                || (!presented_is_owner && argument.access == StructuralAccess::WriteOnlyBorrow)
                || shape != expected_shape
                || u64::from(source_byte_offset)
                    .checked_add(8)
                    .and_then(|end| end.checked_add(capacity))
                    .is_none_or(|end| end > u64::from(source_shape.byte_size))
            {
                return Err(argument_type_mismatch(callee, argument.place));
            }
            return Ok(projected_argument());
        }
        let projected_parameter_shape =
            crate::structural_signature::StructuralCallSignature::parameter_shape(
                crate::structural_signature::ParameterLane::Internal,
                projected_type,
                projected_shape,
                callee_parameter.access,
                structural_types,
            );
        let unchanged_presentation = if argument.access == StructuralAccess::Owned {
            projected_type == callee_parameter.structural_type
        } else {
            matches!(
                terminal_semantics::borrowed_referent_presentation(
                    structural_types.values().copied(),
                    projected_type,
                    callee_parameter.structural_type
                ),
                Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            )
        };
        if !unchanged_presentation
            || argument.access != callee_parameter.access
            || projected_parameter_shape != shape
            || projected_shape
                .byte_size
                .checked_add(source_byte_offset)
                .is_none_or(|end| end > source_shape.byte_size)
        {
            return Err(argument_type_mismatch(callee, argument.place));
        }
        Ok(projected_argument())
    }

    fn owned_owner_home(
        argument: &terminal_psi::StructuralArgument,
        callee_parameter: &terminal_psi::StructuralParameterDeclaration,
        function: &AbstractFunction,
        local_homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Option<OperationId> {
        if !argument.path.is_empty()
            || argument.access != callee_parameter.access
            || !super::super::scalar::byte_views::is_owner_loan(callee_parameter, structural_types)
        {
            return None;
        }
        let (producer, result) = local_homes.get(&argument.place)?.operation_result()?;
        (result.place == argument.place
            && result.structural_type == callee_parameter.structural_type
            && result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.projected_qualifications.is_empty()
            && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && function.operations.iter().any(|operation| {
                matches!(operation,
                abstract_operations::AbstractOperation::EstablishByteSequenceConcatenation {
                    psi_operation, result, ..
                } if *psi_operation == producer && result.place == argument.place)
            }))
        .then_some(producer)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn local_array_loan(
        argument: &terminal_psi::StructuralArgument,
        callee_parameter: &terminal_psi::StructuralParameterDeclaration,
        shape: ValueShape,
        destination: &ValuePlacement,
        callee: MachineId,
        home: &TargetStructuralHomeRequirement,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Result<Option<TargetStructuralArgument>, LoweringError> {
        let Some((producer, result)) = home.operation_result() else {
            return Ok(None);
        };
        if result.place != argument.place
            || !matches!(
                argument.access,
                terminal_psi::reading_borrow_pair_pattern!()
            )
            || result.multiplicity == StructuralMultiplicity::Linear
            || !result.claims.is_empty()
        {
            return Ok(None);
        }
        let presented = terminal_psi::StructuralParameterDeclaration {
            place: result.place,
            position: callee_parameter.position,
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: result.qualifications.clone(),
            projected_qualifications: result.projected_qualifications.clone(),
            additional_input_qualifications: result.additional_input_qualifications.clone(),
        };
        fixed_array_view_argument(
            argument,
            callee_parameter,
            shape,
            destination,
            callee,
            &presented,
            home.layout.shape().byte_size,
            TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
            structural_types,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fixed_array_view_argument(
        argument: &terminal_psi::StructuralArgument,
        callee_parameter: &terminal_psi::StructuralParameterDeclaration,
        shape: ValueShape,
        destination: &ValuePlacement,
        callee: MachineId,
        root: &terminal_psi::StructuralParameterDeclaration,
        root_byte_size: u32,
        source: TargetStructuralArgumentSource,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Result<Option<TargetStructuralArgument>, LoweringError> {
        let mismatch = || LoweringError::StructuralCallArgumentTypeMismatch {
            callee,
            place: argument.place,
        };
        if argument.path.is_empty()
            && root.structural_type == callee_parameter.structural_type
            && structural_types
                .get(root.structural_type)
                .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
        {
            return Ok(None);
        }
        let too_large = || LoweringError::StructuralTypeTooLarge(root.structural_type);
        let (source_byte_offset, length, stride, backing_end) = if let Some(window) =
            terminal_semantics::fixed_byte_array_window(
                structural_types.values().copied(),
                root,
                argument,
                callee_parameter,
            ) {
            let backing_offset = if window.backing_path.is_empty() {
                0
            } else {
                resolve_structural_projection_path(
                    root.structural_type,
                    window.backing_path,
                    structural_types,
                )?
                .2
            };
            let offset = u64::from(backing_offset)
                .checked_add(window.offset)
                .and_then(|offset| u32::try_from(offset).ok())
                .ok_or_else(too_large)?;
            let stored_extent = if argument.path.is_empty()
                && matches!(
                    structural_types
                        .get(root.structural_type)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    ))
                ) {
                8
            } else {
                window.backing_length
            };
            let end = u64::from(backing_offset).checked_add(stored_extent);
            (offset, window.length, 1, end)
        } else if let Some(length) = terminal_semantics::fixed_element_array_extent(
            structural_types.values().copied(),
            root,
            argument,
            callee_parameter,
        ) {
            let (array_type, backing_offset) = if argument.path.is_empty() {
                (root.structural_type, 0)
            } else {
                let (array_type, _, backing_offset) = resolve_structural_projection_path(
                    root.structural_type,
                    &argument.path,
                    structural_types,
                )?;
                (array_type, backing_offset)
            };
            let Some(StructuralTypeShape::FixedArray { element, .. }) = structural_types
                .get(array_type)
                .map(|declaration| &declaration.shape)
            else {
                return Err(mismatch());
            };
            let element_shape = structural_shape(*element, structural_types)?;
            let stride =
                checked_align_up_u32(element_shape.byte_size, u32::from(element_shape.alignment))
                    .ok_or_else(too_large)?;
            let end = u64::from(stride)
                .checked_mul(length)
                .and_then(|extent| u64::from(backing_offset).checked_add(extent));
            (backing_offset, length, stride, end)
        } else {
            return Ok(None);
        };
        let carrier = structural_shape(callee_parameter.structural_type, structural_types)?;
        if shape != ValueShape::borrowed_reference(carrier.byte_size, carrier.alignment)
            || backing_end.is_none_or(|end| end > u64::from(root_byte_size))
        {
            return Err(mismatch());
        }
        let unchanged = argument.path.is_empty()
            && terminal_semantics::borrowed_referent_presentation(
                structural_types.values().copied(),
                root.structural_type,
                callee_parameter.structural_type,
            ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged);
        Ok(Some(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: root.structural_type,
            structural_type: callee_parameter.structural_type,
            shape,
            source_byte_offset,
            fixed_array_length: (!unchanged).then_some(length),
            element_stride: (!unchanged).then_some(stride),
            indices: Vec::new(),
            source,
            destination: destination.clone(),
        }))
    }
}
pub(crate) mod scalar_definitions {
    use super::known_integers::{KnownUnitInteger, insert_known_unit_integer};
    use crate::LoweringError;
    use abstract_operations::{AbstractOperation, AbstractParameter};
    use semantic_vocabulary::{
        IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
    };
    use std::collections::BTreeMap;
    use target_operations::{
        ScalarAbiValue, TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
        TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
    };

    pub(crate) fn lower_integer_widen(
        operation: &AbstractOperation,
        machine: MachineId,
        parameters: &[ScalarAbiValue],
        nonreturning_boundary: bool,
        values: &mut BTreeMap<ValueId, KnownUnitInteger>,
        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        let AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } = operation
        else {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
        };
        let invalid = || LoweringError::IntegerWidenTypeMismatch(*result);
        if nonreturning_boundary || !source_type.can_widen_to(*target_type) {
            return Err(invalid());
        }
        let source_shape = super::super::scalar_abi::fixed_native_integer_shape(*source_type)
            .ok_or_else(invalid)?;
        let result_shape = super::super::scalar_abi::fixed_native_integer_shape(*target_type)
            .ok_or_else(invalid)?;
        let known = values
            .get(operand)
            .copied()
            .ok_or(LoweringError::UnknownValue(*operand))?;
        let operand_expression = match known {
            KnownUnitInteger::BlockParameter {
                block,
                value,
                scalar_type,
            } => {
                if value != *operand || scalar_type != *source_type {
                    return Err(invalid());
                }
                TargetIntegerExpression::BlockParameter(target_operations::TargetScalarBlockValue {
                    block,
                    value,
                    scalar_type: ScalarType::Integer(scalar_type),
                })
            }
            KnownUnitInteger::Parameter {
                parameter_index,
                scalar_type,
            } => {
                let parameter_index = usize::try_from(parameter_index).map_err(|_| invalid())?;
                let parameter = parameters.get(parameter_index).ok_or_else(invalid)?;
                if scalar_type != *source_type
                    || parameter.value != *operand
                    || parameter.scalar_type != ScalarType::Integer(*source_type)
                {
                    return Err(invalid());
                }
                TargetIntegerExpression::Parameter {
                    source_value: *operand,
                    parameter_index,
                    location: super::super::scalar::scalar_parameter_location(
                        &AbstractParameter {
                            value: *operand,
                            scalar_type: parameter.scalar_type,
                        },
                        &parameter.placement,
                    )?,
                }
            }
            KnownUnitInteger::Immediate {
                scalar_type, value, ..
            } => {
                if scalar_type != *source_type {
                    return Err(invalid());
                }
                TargetIntegerExpression::Immediate {
                    source_value: *operand,
                    value,
                }
            }
            KnownUnitInteger::Home(home) => {
                if home.source_value != *operand
                    || home.scalar_type != ScalarType::Integer(*source_type)
                    || home.shape != source_shape
                {
                    return Err(invalid());
                }
                TargetIntegerExpression::ScalarHome(home)
            }
        };
        let result_home = TargetUnitScalarHomeRequirement {
            defining_operation: *psi_operation,
            source_value: *result,
            scalar_type: ScalarType::Integer(*target_type),
            shape: result_shape,
        };
        insert_known_unit_integer(values, *result, KnownUnitInteger::Home(result_home))?;
        operations.push(TargetUnitOperation::ScalarDefinition {
            result_home,
            expression: TargetScalarExpression::Integer {
                scalar_type: *target_type,
                expression: TargetIntegerExpression::IntegerWiden {
                    psi_operation: *psi_operation,
                    source_type: *source_type,
                    operand: Box::new(operand_expression),
                },
            },
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_integer_constant(
        machine: MachineId,
        psi_operation: OperationId,
        result: ValueId,
        scalar_type: IntegerType,
        value: IntegerValue,
        nonreturning_boundary: bool,
        integer_constants: &mut BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
        scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        if nonreturning_boundary {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
        }
        if !scalar_type.admits(value) {
            return Err(LoweringError::IntegerConstantOutsideType(result));
        }
        insert_known_unit_integer(
            scalar_values,
            result,
            KnownUnitInteger::Immediate {
                defining_operation: psi_operation,
                scalar_type,
                value,
            },
        )?;
        if integer_constants
            .insert(result, (psi_operation, scalar_type, value))
            .is_some()
        {
            return Err(LoweringError::DuplicateValue(result));
        }
        operations.push(TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        });
        provenance.operations.push(psi_operation);
        Ok(())
    }

    pub(crate) fn lower_boolean_constant(
        machine: MachineId,
        psi_operation: OperationId,
        result: ValueId,
        value: bool,
        nonreturning_boundary: bool,
        boolean_constants: &mut BTreeMap<ValueId, (OperationId, bool)>,
        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        if nonreturning_boundary
            || boolean_constants
                .insert(result, (psi_operation, value))
                .is_some()
        {
            return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
        }
        operations.push(TargetUnitOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        });
        provenance.operations.push(psi_operation);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_ieee_float_constant(
        psi_operation: OperationId,
        result: ValueId,
        value: semantic_vocabulary::IeeeFloatValue,
        nonreturning_boundary: bool,
        ieee_float_constants: &mut BTreeMap<
            ValueId,
            (OperationId, semantic_vocabulary::IeeeFloatValue),
        >,
        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        if nonreturning_boundary
            || ieee_float_constants
                .insert(result, (psi_operation, value))
                .is_some()
        {
            return Err(LoweringError::DuplicateValue(result));
        }
        operations.push(TargetUnitOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        });
        provenance.operations.push(psi_operation);
        Ok(())
    }
}
mod structural_scalar {
    use crate::LoweringError;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use semantic_vocabulary::PlaceId;
    use std::collections::BTreeMap;
    use target_operations::{
        TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource,
        TerminalPsiProvenance,
    };
    use terminal_psi::StructuralAccess;

    use super::super::scalar::scalar_shape;
    use super::super::scalar_abi::fixed_native_scalar_shape;
    use super::super::structural_layout::{
        direct_scalar_field_geometry, runtime_projection, static_field_geometry,
    };
    use crate::control_flow::scalar_sources::{resolved_source, runtime_indices};
    use abstract_operations::StructuralScalarStoreDestination;
    use target_operations::TargetStructuralArgumentSource;

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_field_store(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
        structural_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
        reference_destination: Option<crate::control_flow::references::ReferentProjection>,
        scalar_sources: &crate::control_flow::scalar_sources::ScalarSources<'_>,

        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        let AbstractOperation::StructuralScalarFieldStore {
            psi_operation,
            destination,
            path,
            field,
            value,
            ..
        } = operation
        else {
            unreachable!("projected field-store lowering receives only field stores")
        };
        let unsupported = |reason| LoweringError::InvalidStructuralValueInput {
            machine: function.machine,
            place: destination.place(),
            reason,
        };
        let (destination, path, destination_storage) = match destination {
            abstract_operations::StructuralScalarStoreDestination::Parameter(destination) => {
                if function.arrival_parameter(destination.place) != Some(destination)
                    || !matches!(
                        destination.access,
                        StructuralAccess::Owned
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    )
                {
                    return Err(unsupported(
                        "scalar field parameter access or unaffected membership does not match its declaration",
                    ));
                }
                if function.structural_parameters.contains(destination) {
                    let parameter = parameters_by_place
                        .get(&destination.place)
                        .copied()
                        .filter(|parameter| {
                            parameter.structural_type == destination.structural_type
                                && parameter.multiplicity == destination.multiplicity
                                && parameter.access == destination.access
                                && parameter.projected_qualifications
                                    == destination.projected_qualifications
                        })
                        .ok_or(unsupported(
                            "scalar field parameter has no matching native placement",
                        ))?;
                    (
                        StructuralScalarStoreDestination::Parameter(destination.clone()),
                        path.clone(),
                        TargetStructuralArgumentSource::Placement(parameter.placement.clone()),
                    )
                } else {
                    let block = function
                        .block_entries
                        .iter()
                        .find(|block| block.structural_parameters.contains(destination))
                        .ok_or(unsupported(
                            "scalar field parameter has no matching block declaration",
                        ))?;
                    (
                        StructuralScalarStoreDestination::Parameter(destination.clone()),
                        path.clone(),
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block: block.block,
                            place: destination.place,
                        },
                    )
                }
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                producer,
                result,
            } => {
                if let Some(resolved) = reference_destination.as_ref() {
                    if resolved.carrier_type != result.structural_type
                        || !structural_homes.get(&result.place).is_some_and(|home| {
                            home.operation_result() == Some((*producer, result))
                        })
                    {
                        return Err(unsupported(
                            "returned reference has no matching declared carrier home",
                        ));
                    }
                    (
                        destination.clone(),
                        path.clone(),
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: *producer,
                        },
                    )
                } else {
                    structural_homes
                        .get(&result.place)
                        .filter(|home| home.operation_result() == Some((*producer, result)))
                        .ok_or(unsupported(
                            "scalar field result has no matching storage home",
                        ))?;
                    (
                        destination.clone(),
                        path.clone(),
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: *producer,
                        },
                    )
                }
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                block,
                parameter,
            } => {
                super::owned_store_home::block_home(structural_homes, *block, parameter)
                    .ok_or_else(|| {
                        unsupported("owned scalar arrival has no matching storage home")
                    })?;
                (
                    destination.clone(),
                    path.clone(),
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: *block,
                        place: parameter.place,
                    },
                )
            }
        };
        let (projection_root, projection_path) = reference_destination
            .as_ref()
            .map(|reference| (reference.referent, reference.path.as_slice()))
            .unwrap_or((destination.structural_type(), path.as_slice()));
        if !terminal_psi::is_structural_scalar_store_path(projection_path) {
            return Err(unsupported(
                "scalar field destination is not an admitted scalar projection",
            ));
        }
        let destination_shape =
            super::super::structural_layout::structural_shape(projection_root, structural_types)?;
        let source = resolved_source(value.value, function, scalar_sources)?;
        scalar_shape(value.value, value.scalar_type, false)?;
        let exact_source = match source {
            TargetUnitScalarArgumentSource::BlockParameter(parameter) => {
                function.block_entries.iter().any(|entry| {
                    entry.block == parameter.block
                        && entry.parameters.iter().any(|declared| {
                            declared.value == parameter.value
                                && declared.scalar_type == parameter.scalar_type
                        })
                })
            }
            TargetUnitScalarArgumentSource::Parameter {
                parameter_index, ..
            } => usize::try_from(parameter_index)
                .ok()
                .and_then(|position| function.parameters.get(position))
                .is_some_and(|parameter| {
                    parameter.value == value.value && parameter.scalar_type == value.scalar_type
                }),
            TargetUnitScalarArgumentSource::Home(home) => {
                Some(home.shape) == fixed_native_scalar_shape(value.scalar_type)
            }
            TargetUnitScalarArgumentSource::IntegerImmediate { .. }
            | TargetUnitScalarArgumentSource::BooleanImmediate { .. }
            | TargetUnitScalarArgumentSource::IeeeFloatImmediate { .. } => true,
        };
        if !exact_source
            || source.source_value() != value.value
            || source.scalar_type() != value.scalar_type
        {
            return Err(unsupported(
                "scalar field source does not match its exact dominating value",
            ));
        }
        let (carrier_path, namespace) = match projection_path.split_last() {
            Some((terminal_psi::StructuralPathSegment::Case(_), prefix)) => {
                (prefix, &projection_path[projection_path.len() - 1..])
            }
            _ => (projection_path, &[][..]),
        };
        let (carrier_type, _, carrier_byte_offset, runs) =
            runtime_projection(projection_root, carrier_path, structural_types)?;
        let indices = runtime_indices(runs, function, scalar_sources)?;
        let (scalar_byte_offset, field_byte_size) = if namespace.is_empty() {
            direct_scalar_field_geometry(carrier_type, *field, value.scalar_type, structural_types)?
        } else {
            static_field_geometry(
                carrier_type,
                namespace,
                *field,
                value.scalar_type,
                structural_types,
            )?
        };
        let destination_is_view = matches!(
            structural_types
                .get(destination.structural_type())
                .map(|declaration| &declaration.shape),
            Some(
                terminal_psi::StructuralTypeShape::ElementView { .. }
                    | terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
            )
        );
        let field_byte_offset = carrier_byte_offset
            .checked_add(scalar_byte_offset)
            .filter(|offset| {
                offset
                    .checked_add(field_byte_size)
                    .is_some_and(|end| destination_is_view || end <= destination_shape.byte_size)
            })
            .ok_or(LoweringError::StructuralTypeTooLarge(
                destination.structural_type(),
            ))?;
        operations.push(TargetUnitOperation::StructuralScalarFieldStore {
            psi_operation: *psi_operation,
            destination,
            path,
            field: *field,
            destination_storage,
            field_byte_offset,
            source,
            indices,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}
pub(crate) use structural_scalar::lower_field_store;
pub(crate) mod write_only_primitive_store {
    use super::super::scalar_abi::fixed_native_integer_shape;
    use super::known_integers::KnownUnitInteger;
    use crate::LoweringError;
    use crate::control_flow::scalar_sources::runtime_indices;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use calling_conventions::ValueShape;
    use semantic_vocabulary::IeeeFloatValue;
    use semantic_vocabulary::{IeeeFloatFormat, OperationId, PlaceId, ScalarType, ValueId};
    use std::collections::BTreeMap;
    use target_operations::TargetStructuralArgumentSource;
    use target_operations::TargetUnitWriteOnlyPrimitiveStoreSource;
    use target_operations::{
        TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarHomeRequirement,
        TerminalPsiProvenance,
    };
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_write_only_primitive_store(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
        structural_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
        scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
        boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
        ieee_float_constants: &BTreeMap<ValueId, (OperationId, IeeeFloatValue)>,
        scalar_homes: &BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
        block_value: Option<target_operations::TargetScalarBlockValue>,
        scalar_sources: &crate::control_flow::scalar_sources::ScalarSources<'_>,
        operations: &mut Vec<TargetUnitOperation>,
        provenance: &mut TerminalPsiProvenance,
    ) -> Result<(), LoweringError> {
        let AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            path,
            value,
            ..
        } = operation
        else {
            unreachable!("whole-root store lowering receives only primitive stores")
        };
        let invalid = || LoweringError::UnsupportedWriteOnlyPrimitiveStore {
            machine: function.machine,
            operation: *psi_operation,
        };
        let destination_storage = match destination {
            abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                if parameter.multiplicity == StructuralMultiplicity::Linear
                    || (path.is_empty()
                        && parameter.multiplicity != StructuralMultiplicity::Unrestricted)
                    || !matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::Owned
                            | terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    )
                {
                    return Err(invalid());
                }
                if let Some(entry) = function.block_entries.iter().find(|entry| {
                    entry.structural_parameters.contains(parameter)
                        && matches!(
                            parameter.access,
                            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                        )
                }) {
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: entry.block,
                        place: parameter.place,
                    }
                } else {
                    let root_shape = super::super::structural_layout::structural_shape(
                        parameter.structural_type,
                        structural_types,
                    )?;
                    let expected_shape =
                        crate::structural_signature::StructuralCallSignature::parameter_shape(
                            crate::structural_signature::ParameterLane::Internal,
                            parameter.structural_type,
                            root_shape,
                            parameter.access,
                            structural_types,
                        );
                    let target_parameter = parameters_by_place
                        .get(&parameter.place)
                        .copied()
                        .filter(|target| {
                            function.structural_parameters.contains(parameter)
                                && target.structural_type == parameter.structural_type
                                && target.multiplicity == parameter.multiplicity
                                && target.access == parameter.access
                                && target.projected_qualifications
                                    == parameter.projected_qualifications
                                && target.shape == expected_shape
                                && target.placement.shape == expected_shape
                        })
                        .ok_or_else(invalid)?;
                    TargetStructuralArgumentSource::Placement(target_parameter.placement.clone())
                }
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                producer,
                result,
            } => {
                let home = structural_homes
                    .get(&result.place)
                    .filter(|home| home.operation_result() == Some((*producer, result)))
                    .ok_or_else(invalid)?;
                if home.has_claims()
                    || home.multiplicity() == StructuralMultiplicity::Linear
                    || (path.is_empty()
                        && home.multiplicity() != StructuralMultiplicity::Unrestricted)
                {
                    return Err(invalid());
                }
                target_operations::TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: *producer,
                }
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                block,
                parameter,
            } => {
                let home = super::owned_store_home::block_home(structural_homes, *block, parameter)
                    .ok_or_else(invalid)?;
                if home.has_claims()
                    || home.multiplicity() == StructuralMultiplicity::Linear
                    || (path.is_empty()
                        && home.multiplicity() != StructuralMultiplicity::Unrestricted)
                {
                    return Err(invalid());
                }
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: *block,
                    place: parameter.place,
                }
            }
        };
        let destination_type = structural_types
            .get(destination.structural_type())
            .copied()
            .ok_or_else(invalid)?;
        let projection = super::super::structural_layout::primitive_leaf_projection(
            destination.structural_type(),
            path,
            structural_types,
        )
        .ok_or_else(invalid)?;
        if projection.scalar != value.scalar_type {
            return Err(invalid());
        }
        let runs = projection.selector_runs();
        let indices = runtime_indices(runs, function, scalar_sources).map_err(|_| invalid())?;
        let (_, source) = if let Some(block_value) = block_value {
            if block_value.value != value.value || block_value.scalar_type != value.scalar_type {
                return Err(invalid());
            }
            let shape = super::super::scalar_abi::fixed_native_scalar_shape(value.scalar_type)
                .ok_or_else(invalid)?;
            (
                ValueShape::borrowed_reference(shape.byte_size, shape.alignment),
                TargetUnitWriteOnlyPrimitiveStoreSource::BlockParameter(block_value),
            )
        } else {
            match value.scalar_type {
                ScalarType::Integer(integer_type) => {
                    let referent_shape =
                        fixed_native_integer_shape(integer_type).ok_or_else(invalid)?;
                    let Some(known_value) = scalar_values.get(&value.value).copied() else {
                        return Err(invalid());
                    };
                    if known_value.scalar_type() != integer_type {
                        return Err(invalid());
                    }
                    let source = match known_value {
                        KnownUnitInteger::Parameter {
                            parameter_index,
                            scalar_type,
                        } => TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                            parameter_index,
                            source_value: value.value,
                            scalar_type: ScalarType::Integer(scalar_type),
                        },
                        KnownUnitInteger::Immediate {
                            defining_operation,
                            scalar_type,
                            value: immediate,
                        } => TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                            defining_operation,
                            source_value: value.value,
                            scalar_type,
                            value: immediate,
                        },
                        KnownUnitInteger::Home(home) => {
                            TargetUnitWriteOnlyPrimitiveStoreSource::Home(home)
                        }
                        KnownUnitInteger::BlockParameter {
                            block,
                            value: source_value,
                            scalar_type,
                        } => {
                            if source_value != value.value {
                                return Err(invalid());
                            }
                            TargetUnitWriteOnlyPrimitiveStoreSource::BlockParameter(
                                target_operations::TargetScalarBlockValue {
                                    block,
                                    value: source_value,
                                    scalar_type: ScalarType::Integer(scalar_type),
                                },
                            )
                        }
                    };
                    (
                        ValueShape::borrowed_reference(
                            referent_shape.byte_size,
                            referent_shape.alignment,
                        ),
                        source,
                    )
                }
                ScalarType::Boolean => {
                    let source = if let Some((parameter_index, _)) = function
                        .parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| {
                            parameter.value == value.value
                                && parameter.scalar_type == ScalarType::Boolean
                        }) {
                        TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                            parameter_index: u32::try_from(parameter_index)
                                .map_err(|_| invalid())?,
                            source_value: value.value,
                            scalar_type: ScalarType::Boolean,
                        }
                    } else if let Some(home) = scalar_homes.get(&value.value) {
                        if home.source_value != value.value
                            || home.scalar_type != ScalarType::Boolean
                            || home.shape != ValueShape::integer(1, 1)
                        {
                            return Err(invalid());
                        }
                        TargetUnitWriteOnlyPrimitiveStoreSource::Home(*home)
                    } else {
                        let (defining_operation, immediate) = boolean_constants
                            .get(&value.value)
                            .copied()
                            .ok_or_else(invalid)?;
                        TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                            defining_operation,
                            source_value: value.value,
                            value: immediate,
                        }
                    };
                    (ValueShape::borrowed_reference(1, 1), source)
                }
                ScalarType::IeeeFloat(format) => {
                    let byte_size = match format {
                        IeeeFloatFormat::Binary32 => 4,
                        IeeeFloatFormat::Binary64 => 8,
                    };
                    let source = if let Some((parameter_index, _)) = function
                        .parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| {
                            parameter.value == value.value
                                && parameter.scalar_type == value.scalar_type
                        }) {
                        TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                            parameter_index: u32::try_from(parameter_index)
                                .map_err(|_| invalid())?,
                            source_value: value.value,
                            scalar_type: value.scalar_type,
                        }
                    } else if let Some(home) = scalar_homes.get(&value.value) {
                        if home.source_value != value.value
                            || home.scalar_type != value.scalar_type
                            || home.shape != ValueShape::float(byte_size)
                        {
                            return Err(invalid());
                        }
                        TargetUnitWriteOnlyPrimitiveStoreSource::Home(*home)
                    } else {
                        let (defining_operation, immediate) = ieee_float_constants
                            .get(&value.value)
                            .copied()
                            .ok_or_else(invalid)?;
                        if immediate.format() != format {
                            return Err(invalid());
                        }
                        TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
                            defining_operation,
                            source_value: value.value,
                            value: immediate,
                        }
                    };
                    (
                        ValueShape::borrowed_reference(u32::from(byte_size), byte_size),
                        source,
                    )
                }
            }
        };
        operations.push(TargetUnitOperation::WriteOnlyPrimitiveStore {
            psi_operation: *psi_operation,
            destination: destination.clone(),
            path: path.clone(),
            destination_type: destination_type.clone(),
            destination_storage,
            source,
            indices,
        });
        provenance.operations.push(*psi_operation);
        Ok(())
    }
}
