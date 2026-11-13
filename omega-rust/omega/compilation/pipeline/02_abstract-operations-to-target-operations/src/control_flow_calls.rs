/*
  Direct, borrowed, owned-argument, byte-argument, and selected-boundary call
  lowering inside the common target control-flow graph (`calls`,
  `borrowed_calls`, `owned_arguments`, `byte_arguments`, `call_signature`,
  `selected_boundary_calls`).

  Resolves callee ABI contracts, argument storage backings, descriptor
  relocations, and scalar or structural result homes for `TargetUnitOperation::Call`.
*/

use super::*;
pub(crate) mod borrowed_calls {
    use super::LiveDefinitions;
    use crate::LoweringError;
    use crate::function_signature::PreparedFunctionSignature;
    use crate::scalar::element_views::is_element_view_parameter;
    use crate::structural_layout::reference_projection_regions;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use calling_conventions::ValueShape;
    use semantic_vocabulary::MachineId;
    use target_operations::{
        TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralParameter,
    };
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};
    use terminal_semantics::borrowed_referent_matches;

    fn direct_referent(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        identity: semantic_vocabulary::StructuralTypeId,
        source: TargetStructuralArgumentSource,
        types: &StructuralTypeLookup<'_>,
    ) -> Option<TargetStructuralArgument> {
        borrowed_referent_matches(
            types.values().copied(),
            identity,
            declaration.structural_type,
        )
        .then(|| TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: Vec::new(),
            root_structural_type: identity,
            structural_type: declaration.structural_type,
            shape: destination.shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            indices: Vec::new(),
            source,
            destination: destination.placement.clone(),
        })
    }

    pub(crate) fn argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        function: &AbstractFunction,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        if argument.path.contains(&StructuralPathSegment::Referent) {
            return super::references::reference_call_argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        if live.address_joins.contains(&argument.place)
            || prepared.parameters.iter().any(|source| {
                source.place == argument.place
                    && (!argument.path.is_empty()
                        || source.access == StructuralAccess::Owned
                            && live.owned_arrivals.contains(&argument.place))
            })
        {
            return storage_argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        if let Some(home) = live.structural_homes.get(&argument.place)
            && (!argument.path.is_empty()
                || !super::primitive_storage::is_primitive_reference(declaration, types))
        {
            let source = crate::unit::argument_roots::resolve(
                argument.place,
                function,
                prepared.parameters.iter(),
                &live.structural_homes,
                &live.address_joins,
                &live.block_views,
                types,
            )?
            .source;
            let (projected_type, projected_shape, source_byte_offset, runs) =
                if argument.path.is_empty() {
                    (
                        home.structural_type(),
                        crate::structural_layout::structural_shape(home.structural_type(), types)?,
                        0,
                        Vec::new(),
                    )
                } else {
                    crate::structural_layout::runtime_borrowed_projection(
                        home.structural_type(),
                        &argument.path,
                        argument.access,
                        Some(declaration.structural_type),
                        types,
                    )?
                };
            let inline =
                reference_projection_regions(home.structural_type(), &argument.path, types)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                    .pointer_slots
                    .is_empty();
            if argument.access != declaration.access
                || !matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
                || declaration.multiplicity != StructuralMultiplicity::Unrestricted
                || !terminal_semantics::borrowed_referent_matches(
                    types.values().copied(),
                    projected_type,
                    declaration.structural_type,
                )
                || home.access() != StructuralAccess::Owned
                || crate::structural_signature::StructuralCallSignature::parameter_shape(
                    crate::structural_signature::ParameterLane::Internal,
                    projected_type,
                    projected_shape,
                    declaration.access,
                    types,
                ) != destination.shape
                || inline
                    && projected_shape
                        .byte_size
                        .checked_add(source_byte_offset)
                        .is_none_or(|end| end > home.layout.shape().byte_size)
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            return Ok(TargetStructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
                root_structural_type: home.structural_type(),
                structural_type: declaration.structural_type,
                shape: destination.shape,
                source_byte_offset,
                fixed_array_length: None,
                element_stride: None,
                indices: super::scalar_sources::runtime_indices(
                    runs,
                    function,
                    &super::scalar_sources::ScalarSources::from(live),
                )?,
                source,
                destination: destination.placement.clone(),
            });
        }
        if !argument.path.is_empty()
            || argument.access != declaration.access
            || !super::primitive_storage::is_primitive_reference(declaration, types)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (identity, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
            let (defining_operation, home_result) = home
                .operation_result()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if !function.operations.iter().any(|operation| {
                matches!(operation,
            AbstractOperation::EstablishPrimitiveLocal { psi_operation, result, .. }
            if *psi_operation == defining_operation && result == home_result)
            }) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            (
                home.structural_type(),
                TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                    psi_operation: defining_operation,
                },
            )
        } else {
            let source = prepared
                .parameter(argument.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let allowed = match source.access {
                StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
                StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
                StructuralAccess::WriteOnlyBorrow => {
                    argument.access == StructuralAccess::WriteOnlyBorrow
                }
                StructuralAccess::Owned => false,
            };
            if !allowed {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            (source.structural_type, source.placement.clone().into())
        };
        direct_referent(argument, declaration, destination, identity, source, types)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))
    }

    pub(crate) fn storage_argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        function: &AbstractFunction,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        if !matches!(
            argument.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ) || declaration.access != argument.access
            || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let root = crate::unit::argument_roots::resolve(
            argument.place,
            function,
            prepared.parameters.iter(),
            &live.structural_homes,
            &live.address_joins,
            &live.block_views,
            types,
        )?;
        let root_type = root.declaration.structural_type;
        let access = root.declaration.access;
        let source = root.source;
        let allowed = match access {
            StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow => {
                argument.access == StructuralAccess::WriteOnlyBorrow
            }
            StructuralAccess::Owned => live.owned_arrivals.contains(&argument.place),
        };
        let (selected, referent, offset, runs) =
            crate::structural_layout::runtime_borrowed_projection(
                root_type,
                &argument.path,
                argument.access,
                Some(declaration.structural_type),
                types,
            )?;
        if !allowed
            || selected != declaration.structural_type
            || destination.shape
                != crate::structural_signature::StructuralCallSignature::parameter_shape(
                    crate::structural_signature::ParameterLane::Internal,
                    selected,
                    referent,
                    declaration.access,
                    types,
                )
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let indices = super::scalar_sources::runtime_indices(
            runs,
            function,
            &super::scalar_sources::ScalarSources::from(live),
        )?;
        Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: root_type,
            structural_type: selected,
            shape: destination.shape,
            source_byte_offset: offset,
            fixed_array_length: None,
            element_stride: None,
            indices,
            source,
            destination: destination.placement.clone(),
        })
    }

    pub(crate) fn address_join_window_argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        callee: MachineId,
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        if !live.address_joins.contains(&argument.place) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let root = crate::unit::argument_roots::resolve(
            argument.place,
            function,
            std::iter::empty::<&TargetStructuralParameter>(),
            &live.structural_homes,
            &live.address_joins,
            &live.block_views,
            types,
        )?;
        if let Some(window) = crate::unit::roster_arguments::fixed_array_view_argument(
            argument,
            declaration,
            destination.shape,
            &destination.placement,
            callee,
            &root.declaration,
            root.backing_shape.byte_size,
            root.source.clone(),
            types,
        )? {
            return Ok(window);
        }
        stored_descriptor_argument(
            argument,
            declaration,
            destination,
            &root,
            function,
            live,
            types,
        )
    }

    fn stored_descriptor_argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        root: &crate::unit::argument_roots::StructuralArgumentRoot,
        function: &AbstractFunction,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        let unsupported = || {
            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                eprintln!(
                    "descriptor call storage refusal machine={:?} argument={argument:?} formal={declaration:?} source={:?}",
                    function.machine, root.declaration,
                );
            }
            LoweringError::unsupported_control_flow(function.machine)
        };
        if argument.access != declaration.access
            || declaration.access == StructuralAccess::Owned
            || !matches!(
                declaration.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || !terminal_semantics::descriptor_source_projection_accessible(
                types.values().copied(),
                root.declaration.structural_type,
                root.declaration.multiplicity,
                root.declaration.access,
                &argument.path,
                argument.access,
            )
        {
            return Err(unsupported());
        }
        let regions = crate::structural_layout::reference_projection_regions(
            root.declaration.structural_type,
            &argument.path,
            types,
        )
        .ok_or_else(unsupported)?;
        let (selected, shape, offset, runs) = crate::structural_layout::runtime_storage_projection(
            regions.root,
            regions.path,
            types,
        )?;
        let selected_shape = selected.shape(types).ok_or_else(unsupported)?;
        let destination_type = types
            .get(declaration.structural_type)
            .ok_or_else(unsupported)?;
        let end = u64::from(offset).checked_add(u64::from(shape.byte_size));
        if !terminal_semantics::descriptor_geometry_matches(
            &selected_shape,
            &destination_type.shape,
        ) || destination.shape
            != ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
            || regions.pointer_slots.is_empty()
                && end.is_none_or(|end| end > u64::from(root.backing_shape.byte_size))
        {
            return Err(unsupported());
        }
        let runs = regions.selector_runs(runs);
        let indices = super::scalar_sources::runtime_indices(
            runs,
            function,
            &super::scalar_sources::ScalarSources::from(live),
        )?;
        Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: root.declaration.structural_type,
            structural_type: declaration.structural_type,
            shape: destination.shape,
            source_byte_offset: offset,
            fixed_array_length: None,
            element_stride: None,
            indices,
            source: root.source.clone(),
            destination: destination.placement.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn byte_argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        callee: MachineId,
        function: &AbstractFunction,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        let shared = argument.access == StructuralAccess::SharedBorrow;
        if prepared.parameter(argument.place).is_some()
            || live.structural_homes.contains_key(&argument.place)
            || live.block_views.contains(&argument.place)
            || live.address_joins.contains(&argument.place)
        {
            let root = crate::unit::argument_roots::resolve(
                argument.place,
                function,
                prepared.parameters.iter(),
                &live.structural_homes,
                &live.address_joins,
                &live.block_views,
                types,
            )?;
            if let Some(presentation) = crate::unit::roster_arguments::fixed_array_view_argument(
                argument,
                declaration,
                destination.shape,
                &destination.placement,
                callee,
                &root.declaration,
                root.backing_shape.byte_size,
                root.source,
                types,
            )? {
                return Ok(presentation);
            }
        }
        if !argument.path.is_empty() {
            return self::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        let carrier_size = match types.get(declaration.structural_type).map(|row| &row.shape) {
            Some(terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedView { .. },
            )) => 8,
            _ => 16,
        };
        if argument.access != declaration.access
            || (!crate::scalar::byte_views::is_byte_parameter(declaration, types)
                && !is_element_view_parameter(declaration, types))
            || destination.shape != ValueShape::borrowed_reference(carrier_size, 8)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let (identity, source) = if let Some((producer, result)) =
            crate::scalar::byte_views::completed_descriptor(
                function,
                argument.place,
                types,
                &live.structural_homes,
            ) {
            let access = terminal_semantics::descriptor_results::descriptor_value_access(
                types.values().copied(),
                result.structural_type,
                result.multiplicity,
            )
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if !(argument.access == access
                || access == StructuralAccess::MutableBorrow
                    && argument.access == StructuralAccess::SharedBorrow)
                || live
                    .structural_homes
                    .get(&argument.place)
                    .and_then(|home| home.operation_result())
                    != Some((producer, result))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            (
                result.structural_type,
                TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: producer,
                },
            )
        } else if let Some((producer, structural_type)) = live.views.get(&argument.place) {
            let source =
                if crate::scalar::element_views::is_element_view_parameter(declaration, types) {
                    let values =
                        super::observations::scalar_values(live, &prepared.scalar_parameters)?;
                    let (_, identity, access) = crate::scalar::element_views::resolve_element_view(
                        function,
                        types,
                        &prepared.parameters,
                        live.element_view_backings(),
                        &values,
                        &live.lengths,
                        argument.place,
                        &mut Vec::new(),
                    )?;
                    if identity != *structural_type
                        || !(access == argument.access
                            || access == StructuralAccess::MutableBorrow && shared)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    TargetStructuralArgumentSource::EstablishedElementView {
                        psi_operation: *producer,
                    }
                } else {
                    let values =
                        super::observations::scalar_values(live, &prepared.scalar_parameters)?;
                    let (_, access) = crate::scalar::byte_views::resolve_byte_view(
                        function,
                        types,
                        &prepared.parameters,
                        live.element_view_backings(),
                        &values,
                        &live.lengths,
                        argument.place,
                        &mut Vec::new(),
                    )?;
                    if access != argument.access
                        && !(access == StructuralAccess::MutableBorrow && shared)
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    TargetStructuralArgumentSource::EstablishedByteView {
                        psi_operation: *producer,
                    }
                };
            (*structural_type, source)
        } else if live.block_views.contains(&argument.place)
            || live.owned_arrivals.contains(&argument.place)
        {
            let (entry, parameter) = function
                .block_entries
                .iter()
                .find_map(|entry| {
                    entry
                        .structural_parameter(argument.place)
                        .map(|parameter| (entry, parameter))
                })
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let access =
                terminal_semantics::descriptor_parameter_access(parameter, types.values().copied())
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let allowed = match access {
                StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
                StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
                StructuralAccess::WriteOnlyBorrow => {
                    argument.access == StructuralAccess::WriteOnlyBorrow
                }
                StructuralAccess::Owned => false,
            };
            let owned_transfer = argument.access == StructuralAccess::Owned
                && parameter.access == StructuralAccess::Owned
                && parameter.multiplicity == declaration.multiplicity
                && terminal_semantics::descriptor_value_transfer_matches(
                    types.values().copied(),
                    parameter.structural_type,
                    declaration.structural_type,
                    parameter.multiplicity,
                );
            if !allowed && !owned_transfer {
                return Err(LoweringError::StructuralStateBindingMismatch {
                    machine: function.machine,
                    reason: format!(
                        "descriptor argument={argument:?}; source={parameter:?}; destination={declaration:?}",
                    ),
                });
            }
            (
                parameter.structural_type,
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place: parameter.place,
                },
            )
        } else {
            let source = prepared
                .parameter(argument.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if source.access != argument.access
                || source.multiplicity != declaration.multiplicity
                || !terminal_semantics::view_descriptor_projected_qualifications(
                    types.values().copied(),
                    source.structural_type,
                    &source.projected_qualifications,
                )
                || source.shape != destination.shape
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            (source.structural_type, source.placement.clone().into())
        };
        direct_referent(argument, declaration, destination, identity, source, types)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))
    }
}

pub(crate) mod byte_arguments {
    use super::LiveDefinitions;
    use crate::LoweringError;
    use crate::function_signature::PreparedFunctionSignature;
    use crate::structural_layout::reference_projection_regions;
    use crate::structural_layout::runtime_storage_projection;
    use crate::structural_layout::structural_shape;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractFunction;
    use calling_conventions::ValueShape;
    use target_operations::{
        TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeOrigin,
        TargetStructuralParameter,
    };
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeShape,
    };

    pub(crate) fn shared_owner_view(
        argument: &StructuralArgument,
        declaration: &StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        function: &AbstractFunction,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<Option<TargetStructuralArgument>, LoweringError> {
        if argument.access != StructuralAccess::SharedBorrow
            || argument.path.iter().any(|segment| {
                matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::Referent
                        | terminal_psi::StructuralPathSegment::FixedRange { .. }
                )
            })
            || declaration.access != StructuralAccess::SharedBorrow
            || !types
                .get(declaration.structural_type)
                .is_some_and(|declared| declared.shape.is_shared_readable_byte_view())
        {
            return Ok(None);
        }
        let (root_type, access, multiplicity, projected, source) =
            if let Some(home) = live.structural_homes.get(&argument.place) {
                let source = match &home.origin {
                    TargetStructuralHomeOrigin::OperationResult { operation, .. } => {
                        TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: *operation,
                        }
                    }
                    TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
                        TargetStructuralArgumentSource::BlockParameter {
                            block: *block,
                            place: declaration.place,
                        }
                    }
                };
                (
                    home.structural_type(),
                    home.access(),
                    home.multiplicity(),
                    home.projected_qualifications(),
                    source,
                )
            } else if let Some(arrival) = function.arrival_parameter(argument.place) {
                let source = if let Some(parameter) = prepared.parameter(argument.place) {
                    parameter.placement.clone().into()
                } else if live.address_joins.contains(&argument.place) {
                    let block = function
                        .block_entries
                        .iter()
                        .find(|block| {
                            block
                                .structural_parameters
                                .iter()
                                .any(|parameter| parameter == arrival)
                        })
                        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                    TargetStructuralArgumentSource::BlockParameter {
                        block: block.block,
                        place: arrival.place,
                    }
                } else {
                    return Ok(None);
                };
                (
                    arrival.structural_type,
                    arrival.access,
                    arrival.multiplicity,
                    arrival.projected_qualifications.as_slice(),
                    source,
                )
            } else {
                return Ok(None);
            };
        if !reference_projection_regions(root_type, &argument.path, types)
            .ok_or(LoweringError::UnsupportedStructuralReference(root_type))?
            .pointer_slots
            .is_empty()
        {
            return Ok(None);
        }
        let (selected, _, offset, runs) =
            runtime_storage_projection(root_type, &argument.path, types)?;
        let Some(selected_shape) = selected.shape(types) else {
            return Ok(None);
        };
        let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            *selected_shape
        else {
            return Ok(None);
        };
        if live
            .structural_homes
            .get(&argument.place)
            .is_some_and(|home| {
                home.has_claims()
                    || home
                        .operation_result()
                        .is_some_and(|(_, result)| !result.qualification_establishments.is_empty())
            })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let root_shape = structural_shape(root_type, types)?;
        if !matches!(
            access,
            StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
        ) || multiplicity != StructuralMultiplicity::Unrestricted
            || !projected.is_empty()
            || declaration.multiplicity != StructuralMultiplicity::Unrestricted
            || !declaration.projected_qualifications.is_empty()
            || destination.shape != ValueShape::borrowed_reference(16, 8)
            || destination.placement.shape != destination.shape
            || u64::from(offset)
                .checked_add(8)
                .and_then(|end| end.checked_add(capacity))
                .is_none_or(|end| end > u64::from(root_shape.byte_size))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(Some(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type: root_type,
            structural_type: declaration.structural_type,
            shape: destination.shape,
            source_byte_offset: offset,
            fixed_array_length: None,
            element_stride: None,
            indices: super::scalar_sources::runtime_indices(
                runs,
                function,
                &super::scalar_sources::ScalarSources::from(live),
            )?,
            source,
            destination: destination.placement.clone(),
        }))
    }
}

pub(crate) mod call_signature {
    use abstract_operations::{AbstractCallSignature, StructuralCallFormal};
    use calling_conventions::{ValuePlacement, ValueShape};
    use semantic_vocabulary::ScalarType;
    use target::NativeTarget;

    use crate::LoweringError;
    use crate::structural_signature::{StructuralCallPlan, StructuralCallSignature};
    use crate::structural_type_lookup::StructuralTypeLookup;

    pub(crate) struct PreparedScalarCallFormal {
        pub(crate) scalar_type: ScalarType,
        pub(crate) placement: ValuePlacement,
    }

    pub(crate) struct PreparedStructuralCallFormal {
        pub(crate) shape: ValueShape,
        pub(crate) placement: ValuePlacement,
    }

    pub(crate) struct PreparedCallSignature {
        pub(crate) call_plan: StructuralCallPlan,
        pub(crate) scalar_parameters: Vec<PreparedScalarCallFormal>,
        pub(crate) parameters: Vec<PreparedStructuralCallFormal>,
    }

    pub(crate) fn prepare(
        machine: semantic_vocabulary::MachineId,
        declaration: AbstractCallSignature<'_>,
        target: NativeTarget,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<PreparedCallSignature, LoweringError> {
        let scalar_types = (0..declaration.scalar_parameter_count())
            .map(|position| {
                declaration.scalar_parameter(position).ok_or(
                    LoweringError::AbiParameterCountMismatch {
                        expected: declaration.scalar_parameter_count(),
                        actual: position,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scalar_shapes = scalar_types
            .iter()
            .map(|scalar_type| {
                crate::scalar_abi::fixed_native_scalar_shape(*scalar_type)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let formals = (0..declaration.structural_parameter_count())
            .map(|position| {
                declaration
                    .structural_parameter(position)
                    .map(|parameter: StructuralCallFormal<'_>| {
                        (parameter.structural_type, parameter.access)
                    })
                    .ok_or(LoweringError::AbiParameterCountMismatch {
                        expected: declaration.structural_parameter_count(),
                        actual: position,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = if let Some(result) = declaration.structural_result() {
            Some(
                super::aggregate_results::value_home_layout(result.structural_type, types)?.shape(),
            )
        } else {
            declaration
                .scalar_result()
                .map(|scalar_type| {
                    crate::scalar_abi::fixed_native_scalar_shape(scalar_type)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(machine))
                })
                .transpose()?
        };
        let signature = StructuralCallSignature::derive(&scalar_shapes, formals, result, types)?;
        let call_plan = signature.plan(target)?;
        let shapes = [scalar_shapes.as_slice(), signature.structural_shapes()].concat();
        if call_plan.parameters.len() != shapes.len() {
            return Err(LoweringError::AbiParameterCountMismatch {
                expected: shapes.len(),
                actual: call_plan.parameters.len(),
            });
        }
        let scalar_parameters = scalar_types
            .into_iter()
            .zip(&call_plan.parameters)
            .map(|(scalar_type, placement)| PreparedScalarCallFormal {
                scalar_type,
                placement: placement.clone(),
            })
            .collect();
        let parameters = shapes
            .into_iter()
            .zip(&call_plan.parameters)
            .skip(declaration.scalar_parameter_count())
            .map(|(shape, placement)| PreparedStructuralCallFormal {
                shape,
                placement: placement.clone(),
            })
            .collect();
        Ok(PreparedCallSignature {
            call_plan,
            scalar_parameters,
            parameters,
        })
    }

    impl From<crate::function_signature::PreparedFunctionSignature> for PreparedCallSignature {
        fn from(signature: crate::function_signature::PreparedFunctionSignature) -> Self {
            Self {
                call_plan: signature.call_plan,
                scalar_parameters: signature
                    .scalar_parameters
                    .into_iter()
                    .map(|parameter| PreparedScalarCallFormal {
                        scalar_type: parameter.scalar_type,
                        placement: parameter.placement,
                    })
                    .collect(),
                parameters: signature
                    .parameters
                    .into_iter()
                    .map(|parameter| PreparedStructuralCallFormal {
                        shape: parameter.shape,
                        placement: parameter.placement,
                    })
                    .collect(),
            }
        }
    }
}

pub(crate) mod calls {
    use super::primitive_storage::retain_result;
    use super::{LiveDefinitions, RowSite};
    use crate::LoweringError;
    use crate::function_signature::{PreparedFunctionSignature, prepare_function_signature};
    use crate::scalar::byte_views::is_byte_parameter;
    use crate::scalar::element_views::is_element_view_parameter;
    use crate::scalar_abi::fixed_native_scalar_shape;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use crate::unit::roster_arguments::lower_structural_argument;
    use abstract_operations::{
        AbstractCallSignature, AbstractCallTarget, AbstractFunction, AbstractFunctionResult,
        AbstractOperation, AbstractResult, AbstractStructuralCallResult, StructuralCallFormal,
    };
    use calling_conventions::ValueLocation;
    use semantic_vocabulary::{
        MachineId, ObligationId, OperationId, PlaceId, StructuralTypeId, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use target::NativeTarget;
    use target_operations::TargetStructuralResultContract;
    use target_operations::{
        ScalarAbiValue, ScalarFunctionAbi, TargetCallResult, TargetStructuralArgument,
        TargetStructuralHomeRequirement, TargetStructuralParameter, TargetUnitOperation,
        TargetUnitScalarCallArgument,
    };
    use terminal_psi::{
        ClaimTransfer, CrashRouteBucket, StructuralAccess, StructuralArgument,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
    };
    use terminal_semantics::descriptor_results::is_descriptor;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum CallContract {
        PublishedScalar,
        Ordinary,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_no_normal(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        target: NativeTarget,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        types: &StructuralTypeLookup<'_>,
        prepared: &PreparedFunctionSignature,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
        live: &LiveDefinitions,
        operations: &[TargetUnitOperation],
        provenance: &mut target_operations::TerminalPsiProvenance,
    ) -> Result<target_operations::TargetControlTerminator, LoweringError> {
        let AbstractOperation::InvokeNoNormal {
            psi_edge,
            callee,
            arguments: scalar_values,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } = operation
        else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let declaration = functions
            .get(callee)
            .copied()
            .ok_or(LoweringError::UnknownCallTarget(*callee))?;
        if scalar_values.len() != declaration.parameters.len() {
            return Err(LoweringError::CallArgumentCountMismatch {
                callee: *callee,
                expected: declaration.parameters.len(),
                actual: scalar_values.len(),
            });
        }
        if structural_arguments.len() != declaration.structural_parameters.len() {
            return Err(LoweringError::StructuralCallArgumentCountMismatch {
                callee: *callee,
                expected: declaration.structural_parameters.len(),
                actual: structural_arguments.len(),
            });
        }
        let signature = super::call_signature::prepare(
            function.machine,
            AbstractCallSignature::Machine(declaration),
            target,
            types,
        )?;
        let scalar_arguments = scalar_values
            .iter()
            .zip(&signature.scalar_parameters)
            .enumerate()
            .map(|(position, (value, parameter))| {
                scalar_argument(*callee, position, *value, parameter, function, live)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let arguments = structural_arguments
            .iter()
            .zip(&declaration.structural_parameters)
            .zip(&signature.parameters)
            .map(|((argument, formal), destination)| {
                structural_argument(
                    argument,
                    &StructuralCallFormal::from(formal),
                    destination,
                    *callee,
                    function,
                    types,
                    prepared,
                    parameters_by_place,
                    live,
                    operations,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let access_bindings = super::references::access_call_custody(
            function,
            declaration,
            semantic_vocabulary::InvocationSite::InvokeNoNormal(*psi_edge),
            structural_arguments,
            types,
            live,
        )?;
        let declared_result = declaration.result.clone();
        provenance.edges.push(*psi_edge);
        Ok(target_operations::TargetControlTerminator::InvokeNoNormal {
            psi_edge: *psi_edge,
            callee: *callee,
            call_plan: signature.call_plan.into_call_plan(),
            scalar_arguments,
            arguments,
            access_bindings,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
            declared_result,
        })
    }

    struct Invocation<'a> {
        psi_operation: OperationId,
        target: AbstractCallTarget,
        diagnostic_owner: MachineId,
        scalar_arguments: &'a [ValueId],
        structural_arguments: &'a [StructuralArgument],
        claim_transfers: &'a [ClaimTransfer],
        requirement_obligations: &'a [ObligationId],
        crash_continuations: &'a [CrashRouteBucket],
        result: &'a AbstractStructuralCallResult,
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_call(
        operation: &AbstractOperation,
        site: &mut RowSite<'_>,
        target: NativeTarget,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        callables: &terminal_psi::TerminalCallableCatalog,
        scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
        prepared: &PreparedFunctionSignature,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    ) -> Result<(), LoweringError> {
        let (function, types, live, operations, provenance) = site.parts();
        let published_result;
        let (call, contract) = match operation {
            AbstractOperation::CallStructural {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                result,
                requirement_obligations,
                crash_continuations,
            } => {
                let call = Invocation {
                    psi_operation: *psi_operation,
                    target: *callee,
                    diagnostic_owner: callee.machine().unwrap_or(function.machine),
                    scalar_arguments: arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                    result,
                };
                (call, CallContract::Ordinary)
            }
            AbstractOperation::Call {
                psi_operation,
                result,
                scalar_type,
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
            } => {
                published_result = AbstractStructuralCallResult::Scalar(AbstractResult {
                    value: *result,
                    scalar_type: *scalar_type,
                });
                let call = Invocation {
                    psi_operation: *psi_operation,
                    target: AbstractCallTarget::Machine(*callee),
                    diagnostic_owner: *callee,
                    scalar_arguments: arguments,
                    structural_arguments: &[],
                    claim_transfers: &[],
                    requirement_obligations,
                    crash_continuations,
                    result: &published_result,
                };
                (call, CallContract::PublishedScalar)
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        let (declaration, callee_function) = match call.target {
            AbstractCallTarget::Machine(machine) => {
                let callee = functions
                    .get(&machine)
                    .copied()
                    .ok_or(LoweringError::UnknownCallTarget(machine))?;
                (AbstractCallSignature::Machine(callee), Some(callee))
            }
            AbstractCallTarget::Callable { callee, contract } => {
                let contract = callables
                    .contract(contract)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let operand = super::scalar_sources::source(callee, function, live)?;
                if operand.scalar_type() != semantic_vocabulary::ScalarType::Callable(contract.id) {
                    return Err(LoweringError::ValueTypeMismatch(callee));
                }
                (AbstractCallSignature::Callable(contract), None)
            }
            AbstractCallTarget::Dynamic {
                interface,
                slot,
                selector,
            } => {
                let requirement = dynamic_dispatch
                    .interface(interface)
                    .and_then(|interface| interface.slots.get(slot as usize))
                    .filter(|requirement| requirement.slot == slot)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let operand = super::scalar_sources::source(selector, function, live)?;
                if !matches!(operand.scalar_type(), semantic_vocabulary::ScalarType::Integer(integer)
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64)
                {
                    return Err(LoweringError::ValueTypeMismatch(selector));
                }
                (AbstractCallSignature::Requirement(requirement), None)
            }
        };
        if callee_function.is_none()
            && (0..declaration.structural_parameter_count()).any(|position| {
                declaration
                    .structural_parameter(position)
                    .is_some_and(|formal| {
                        types
                            .catalog()
                            .access_views()
                            .iter()
                            .any(|plan| plan.carrier == formal.structural_type)
                    })
            })
        {
            return Err(LoweringError::AccessDynamicCallRequiresBinding {
                caller: function.machine,
                operation: call.psi_operation,
            });
        }
        let access_bindings = if let Some(callee) = callee_function {
            super::references::access_call_custody(
                function,
                callee,
                call.psi_operation.into(),
                call.structural_arguments,
                types,
                live,
            )?
        } else {
            Vec::new()
        };
        let (signature, structural_home) = match contract {
            CallContract::PublishedScalar => (
                published_scalar_contract(
                    &call,
                    callee_function
                        .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
                    target,
                    scalar_abis,
                )?
                .into(),
                None,
            ),
            CallContract::Ordinary => {
                ordinary_contract(&call, declaration, function, target, types)?
            }
        };
        let moved = super::references::moved_argument_leaves(live, call.structural_arguments);
        let stored = super::references::stage_stored_effects(
            function,
            call.psi_operation,
            callee_function,
            types,
            live,
            call.structural_arguments,
        )?;
        let scalar_arguments = call
            .scalar_arguments
            .iter()
            .zip(&signature.scalar_parameters)
            .enumerate()
            .map(|(position, (value, parameter))| {
                scalar_argument(
                    call.diagnostic_owner,
                    position,
                    *value,
                    parameter,
                    function,
                    live,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let arguments = call
            .structural_arguments
            .iter()
            .zip(
                (0..declaration.structural_parameter_count())
                    .map(|position| declaration.structural_parameter(position)),
            )
            .zip(&signature.parameters)
            .map(|((argument, declaration), destination)| {
                let declaration =
                    declaration.ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                structural_argument(
                    argument,
                    &declaration,
                    destination,
                    call.diagnostic_owner,
                    function,
                    types,
                    prepared,
                    parameters_by_place,
                    live,
                    operations,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = land_result(
            contract,
            &call,
            declaration,
            structural_home,
            &moved,
            function,
            types,
            live,
        )?;
        super::references::commit_argument_moves(live, &moved);
        super::references::commit_stored_effects(live, stored);
        operations.push(TargetUnitOperation::Call {
            origin: target_operations::NativeCallOrigin::Authored,
            psi_operation: call.psi_operation,
            target: match call.target {
                AbstractCallTarget::Machine(machine) => {
                    target_operations::NativeCallTarget::Machine(machine)
                }
                AbstractCallTarget::Callable { callee, contract } => {
                    target_operations::NativeCallTarget::Callable { callee, contract }
                }
                AbstractCallTarget::Dynamic {
                    selector,
                    interface,
                    slot,
                } => target_operations::NativeCallTarget::Dynamic {
                    selector,
                    interface,
                    slot,
                },
            },
            call_plan: signature.call_plan.into_call_plan(),
            result,
            scalar_arguments,
            arguments,
            access_bindings,
            claim_transfers: call.claim_transfers.to_vec(),
            requirement_obligations: call.requirement_obligations.to_vec(),
            crash_continuations: call.crash_continuations.to_vec(),
        });
        provenance.operations.push(call.psi_operation);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn land_result(
        contract: CallContract,
        call: &Invocation<'_>,
        declaration: AbstractCallSignature<'_>,
        structural_home: Option<TargetStructuralHomeRequirement>,
        moved: &BTreeMap<
            (PlaceId, Vec<StructuralPathSegment>),
            super::references::ReferenceStorage,
        >,
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        live: &mut LiveDefinitions,
    ) -> Result<TargetCallResult, LoweringError> {
        Ok(match call.result {
            AbstractStructuralCallResult::Unit => TargetCallResult::Unit,
            AbstractStructuralCallResult::Scalar(result) => {
                if contract == CallContract::PublishedScalar
                    && super::scalar_sources::source(result.value, function, live).is_ok()
                {
                    return Err(LoweringError::DuplicateValue(result.value));
                }
                TargetCallResult::Scalar(retain_result(call.psi_operation, *result, live)?)
            }
            AbstractStructuralCallResult::Structural { result, .. } => {
                let result_home = structural_home
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                let reference_results = super::references::call_results(
                    function,
                    declaration,
                    types,
                    result,
                    call.structural_arguments,
                    live,
                    moved,
                )?;
                let callee_result = match declaration {
                    AbstractCallSignature::Machine(callee) => {
                        let result_declaration = callee
                            .result
                            .structural()
                            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                        TargetStructuralResultContract::Machine(result_declaration.clone())
                    }
                    AbstractCallSignature::Callable(contract) => {
                        let terminal_psi::BoundaryMachineResult::Structural(result_declaration) =
                            &contract.result
                        else {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        };
                        TargetStructuralResultContract::Callable(result_declaration.clone())
                    }
                    AbstractCallSignature::Requirement(requirement) => {
                        let terminal_psi::BoundaryMachineResult::Structural(result_declaration) =
                            &requirement.result
                        else {
                            return Err(LoweringError::unsupported_control_flow(function.machine));
                        };
                        TargetStructuralResultContract::Requirement(result_declaration.clone())
                    }
                };
                if live
                    .structural_homes
                    .insert(result.place, result_home.clone())
                    .is_some()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                if (bounded_owned_byte_result(result.structural_type, types)
                    || is_descriptor(types.values().copied(), result.structural_type))
                    && live
                        .views
                        .insert(result.place, (call.psi_operation, result.structural_type))
                        .is_some()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                TargetCallResult::Structural {
                    result: result.clone(),
                    callee_result,
                    result_home: Some(result_home),
                    reference_results,
                }
            }
        })
    }

    fn published_scalar_contract(
        call: &Invocation<'_>,
        callee_function: &AbstractFunction,
        target: NativeTarget,
        scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    ) -> Result<PreparedFunctionSignature, LoweringError> {
        let callee = call.diagnostic_owner;
        let unsupported = || LoweringError::UnitScalarCallTargetShapeUnsupported(callee);
        let AbstractStructuralCallResult::Scalar(result) = call.result else {
            return Err(unsupported());
        };
        if !callee_function.structural_parameters.is_empty()
            || !callee_function.entry_claims.is_empty()
        {
            return Err(unsupported());
        }
        let callee_result = callee_function.result.scalar().ok_or_else(unsupported)?;
        let result_shape = fixed_native_scalar_shape(result.scalar_type).ok_or(
            LoweringError::UnitScalarCallIntegerTypeUnsupported(result.value),
        )?;
        if result.scalar_type != callee_result.scalar_type {
            return Err(LoweringError::UnitScalarCallResultTypeMismatch {
                callee,
                result: result.value,
            });
        }
        let parameter_shapes = callee_function
            .parameters
            .iter()
            .map(|parameter| {
                fixed_native_scalar_shape(parameter.scalar_type).ok_or_else(unsupported)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let call_plan = crate::structural_signature::StructuralCallSignature::derive(
            &parameter_shapes,
            [],
            Some(result_shape),
            &calling_conventions::terminal_layout::StructuralLayouts::default(),
        )?
        .plan(target)?;
        if call.scalar_arguments.len() != callee_function.parameters.len()
            || call.scalar_arguments.len() != call_plan.parameters.len()
        {
            return Err(LoweringError::CallArgumentCountMismatch {
                callee,
                expected: callee_function.parameters.len(),
                actual: call.scalar_arguments.len(),
            });
        }
        let placement_unsupported = || LoweringError::UnitScalarCallResultPlacementUnsupported {
            callee,
            result: result.value,
        };
        let result_placement = call_plan.result.clone().ok_or_else(placement_unsupported)?;
        if !matches!(
            result_placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            }] if *byte_size == result_shape.byte_size
        ) {
            return Err(placement_unsupported());
        }
        let expected_abi = ScalarFunctionAbi {
            parameters: callee_function
                .parameters
                .iter()
                .zip(&call_plan.parameters)
                .map(|(parameter, placement)| ScalarAbiValue {
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                    placement: placement.clone(),
                })
                .collect(),
            result: Some(ScalarAbiValue {
                value: callee_result.value,
                scalar_type: callee_result.scalar_type,
                placement: result_placement,
            }),
            call_plan: call_plan.clone().into_call_plan(),
        };
        require_exact_target_abi(callee, scalar_abis.get(&callee), &expected_abi)?;
        Ok(PreparedFunctionSignature {
            call_plan,
            scalar_parameters: expected_abi.parameters,
            parameters: Vec::new(),
        })
    }

    fn require_exact_target_abi(
        callee: MachineId,
        actual: Option<&ScalarFunctionAbi>,
        expected: &ScalarFunctionAbi,
    ) -> Result<(), LoweringError> {
        if actual != Some(expected) {
            return Err(LoweringError::UnitScalarCallTargetAbiMismatch(callee));
        }
        Ok(())
    }

    fn ordinary_contract(
        call: &Invocation<'_>,
        declaration: AbstractCallSignature<'_>,
        function: &AbstractFunction,
        target: NativeTarget,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<
        (
            super::call_signature::PreparedCallSignature,
            Option<TargetStructuralHomeRequirement>,
        ),
        LoweringError,
    > {
        if call.scalar_arguments.len() != declaration.scalar_parameter_count() {
            return Err(LoweringError::CallArgumentCountMismatch {
                callee: call.diagnostic_owner,
                expected: declaration.scalar_parameter_count(),
                actual: call.scalar_arguments.len(),
            });
        }
        if call.structural_arguments.len() != declaration.structural_parameter_count() {
            return Err(LoweringError::StructuralCallArgumentCountMismatch {
                callee: call.diagnostic_owner,
                expected: declaration.structural_parameter_count(),
                actual: call.structural_arguments.len(),
            });
        }
        let home = match call.result {
            AbstractStructuralCallResult::Unit => {
                if !declaration.is_unit_result() {
                    return Err(LoweringError::UnitCallTargetKindMismatch(
                        call.diagnostic_owner,
                    ));
                }
                None
            }
            AbstractStructuralCallResult::Scalar(result) => {
                if declaration.scalar_result() != Some(result.scalar_type) {
                    return Err(LoweringError::UnitScalarCallResultTypeMismatch {
                        callee: call.diagnostic_owner,
                        result: result.value,
                    });
                }
                None
            }
            AbstractStructuralCallResult::Structural { .. } => {
                Some(structural_result_home(call, declaration, function, types)?)
            }
        };
        let signature =
            super::call_signature::prepare(function.machine, declaration, target, types)?;
        Ok((signature, home))
    }

    fn structural_result_home(
        call: &Invocation<'_>,
        declaration: AbstractCallSignature<'_>,
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
    ) -> Result<TargetStructuralHomeRequirement, LoweringError> {
        let AbstractStructuralCallResult::Structural { result, .. } = call.result else {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        };
        let callee_result = declaration
            .structural_result()
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let result_home = super::aggregate_results::home(call.psi_operation, result, types)?;
        if callee_result.structural_type != result.structural_type
            || callee_result.multiplicity != result.multiplicity
            || !result_qualifications_match(callee_result.qualifications, result)
            || !callee_result
                .projected_qualifications
                .iter()
                .all(|row| result.projected_qualifications.contains(row))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(result_home)
    }

    fn result_qualifications_match(
        qualifications: &[semantic_vocabulary::StructuralDomainId],
        result: &terminal_psi::StructuralOperationResult,
    ) -> bool {
        let minted = result
            .qualification_establishments
            .iter()
            .map(|establishment| establishment.domain);
        let expected = qualifications
            .iter()
            .copied()
            .chain(minted)
            .collect::<BTreeSet<_>>();
        let actual = result
            .qualifications
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        actual.len() == result.qualifications.len() && actual == expected
    }

    fn bounded_owned_byte_result(
        structural_type: StructuralTypeId,
        types: &StructuralTypeLookup<'_>,
    ) -> bool {
        types
            .get(structural_type)
            .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
    }

    fn scalar_argument(
        callee: MachineId,
        position: usize,
        value: ValueId,
        parameter: &super::call_signature::PreparedScalarCallFormal,
        function: &AbstractFunction,
        live: &LiveDefinitions,
    ) -> Result<TargetUnitScalarCallArgument, LoweringError> {
        let source = super::scalar_sources::source(value, function, live)?;
        if source.scalar_type() != parameter.scalar_type {
            return Err(LoweringError::CallArgumentTypeMismatch {
                callee,
                argument: value,
            });
        }
        Ok(TargetUnitScalarCallArgument {
            parameter_index: u32::try_from(position)
                .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
            source,
            placement: parameter.placement.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn structural_argument(
        argument: &StructuralArgument,
        formal: &StructuralCallFormal<'_>,
        placement: &super::call_signature::PreparedStructuralCallFormal,
        callee: MachineId,
        function: &AbstractFunction,
        types: &StructuralTypeLookup<'_>,
        prepared: &PreparedFunctionSignature,
        parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
        live: &LiveDefinitions,
        operations: &[TargetUnitOperation],
    ) -> Result<TargetStructuralArgument, LoweringError> {
        let declaration = &StructuralParameterDeclaration {
            place: argument.place,
            position: formal.position,
            structural_type: formal.structural_type,
            multiplicity: formal.multiplicity,
            access: formal.access,
            qualifications: formal.qualifications.to_vec(),
            projected_qualifications: formal.projected_qualifications.to_vec(),
            additional_input_qualifications: formal.additional_input_qualifications.to_vec(),
        };
        let destination = &TargetStructuralParameter {
            place: argument.place,
            structural_type: formal.structural_type,
            multiplicity: formal.multiplicity,
            access: formal.access,
            projected_qualifications: formal.projected_qualifications.to_vec(),
            shape: placement.shape,
            placement: placement.placement.clone(),
        };
        if let Some(argument) = super::byte_arguments::shared_owner_view(
            argument,
            declaration,
            destination,
            function,
            prepared,
            live,
            types,
        )? {
            return Ok(argument);
        }
        if argument.path.contains(&StructuralPathSegment::Referent) {
            return super::references::reference_call_argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        if super::aggregate_borrows::is_reference(declaration, types) {
            return super::aggregate_borrows::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        if super::owned_arguments::is_owned_parameter(declaration, types) {
            return super::owned_arguments::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        if super::primitive_storage::is_primitive_reference(declaration, types)
            && (argument.path.is_empty() || live.structural_homes.contains_key(&argument.place))
        {
            return super::borrowed_calls::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        let view =
            is_byte_parameter(declaration, types) || is_element_view_parameter(declaration, types);
        let invocation_projection = !argument.path.is_empty()
            && !types
                .get(declaration.structural_type)
                .is_some_and(|ty| matches!(ty.shape, StructuralTypeShape::ByteSequence(_)))
            && (prepared.parameters.iter().any(|parameter| {
                parameter.place == argument.place && parameter.access != StructuralAccess::Owned
            }) || live.block_views.contains(&argument.place));
        if !view
            && (live.address_joins.contains(&argument.place) || invocation_projection)
            && argument.access != StructuralAccess::Owned
        {
            return super::borrowed_calls::storage_argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        if live.address_joins.contains(&argument.place) {
            return super::borrowed_calls::address_join_window_argument(
                argument,
                declaration,
                destination,
                callee,
                function,
                live,
                types,
            );
        }
        if live.block_views.contains(&argument.place)
            || (view
                && (live.views.contains_key(&argument.place)
                    || live.structural_homes.contains_key(&argument.place)))
        {
            return super::borrowed_calls::byte_argument(
                argument,
                declaration,
                destination,
                callee,
                function,
                prepared,
                live,
                types,
            );
        }
        if live.structural_homes.contains_key(&argument.place) {
            return super::borrowed_calls::argument(
                argument,
                declaration,
                destination,
                function,
                prepared,
                live,
                types,
            );
        }
        lower_structural_argument(
            argument,
            declaration,
            destination.shape,
            &destination.placement,
            callee,
            function,
            types,
            parameters_by_place,
            &live.structural_homes,
            &live.views,
            operations,
            &super::scalar_sources::ScalarSources::from(live),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_cleanup_calls(
        terminator: &AbstractOperation,
        function: &AbstractFunction,
        target: NativeTarget,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
        types: &StructuralTypeLookup<'_>,
        prepared: &PreparedFunctionSignature,
        live: &LiveDefinitions,
        operations: &mut Vec<TargetUnitOperation>,
    ) -> Result<(), LoweringError> {
        for (edge, actions) in terminator.cleanup_rosters() {
            for (ordinal, action) in actions.iter().enumerate() {
                let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) = action
                else {
                    continue;
                };
                let callee = functions
                    .get(&cleanup.cleanup_machine)
                    .copied()
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                if callee.attachment != Some(cleanup.structural_type)
                    || callee.nominal_cleanup_subject != Some(cleanup.structural_type)
                    || callee.result != AbstractFunctionResult::Unit
                    || !callee.parameters.is_empty()
                    || callee.structural_parameters.len() > 1
                    || !cleanup.requirement_obligations.is_empty()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                let signature = prepare_function_signature(callee, target, types)?;
                let mut arguments = Vec::new();
                for (parameter, destination) in callee
                    .structural_parameters
                    .iter()
                    .zip(&signature.parameters)
                {
                    if !parameter.is_cleanup_receiver(cleanup) {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    let argument = StructuralArgument {
                        place: cleanup.place,
                        path: cleanup.path.clone(),
                        access: parameter.access,
                    };
                    arguments.push(super::borrowed_calls::argument(
                        &argument,
                        parameter,
                        destination,
                        function,
                        prepared,
                        live,
                        types,
                    )?);
                }
                if !signature.scalar_parameters.is_empty()
                    || signature.parameters.len() != callee.structural_parameters.len()
                    || (callee.structural_parameters.is_empty()
                        && cleanup.cleanup_receiver.is_some())
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                operations.push(TargetUnitOperation::CleanupCall {
                    owner: target_operations::CallSiteOwner::CleanupAction {
                        edge,
                        action_ordinal: ordinal.try_into().map_err(|_| {
                            LoweringError::unsupported_control_flow(function.machine)
                        })?,
                    },
                    callee: cleanup.cleanup_machine,
                    call_plan: signature.call_plan.into_call_plan(),
                    arguments,
                });
            }
        }
        Ok(())
    }
}

pub(crate) mod owned_arguments {
    use super::LiveDefinitions;
    use crate::LoweringError;
    use crate::structural_layout::runtime_projection;
    use calling_conventions::terminal_layout::StructuralLayouts;
    use target_operations::{TargetStructuralArgument, TargetStructuralParameter};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};

    pub(crate) fn is_owned_parameter(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        types: &StructuralLayouts<'_>,
    ) -> bool {
        parameter.access == StructuralAccess::Owned
            && super::aggregate_results::value_home_layout(parameter.structural_type, types).is_ok()
    }

    pub(crate) fn argument(
        argument: &terminal_psi::StructuralArgument,
        declaration: &terminal_psi::StructuralParameterDeclaration,
        destination: &TargetStructuralParameter,
        function: &abstract_operations::AbstractFunction,
        prepared: &crate::function_signature::PreparedFunctionSignature,
        live: &LiveDefinitions,
        types: &StructuralLayouts<'_>,
    ) -> Result<TargetStructuralArgument, LoweringError> {
        let invalid = |reason| LoweringError::InvalidOwnedStructuralArgument {
            source: argument.place,
            parameter: declaration.place,
            structural_type: declaration.structural_type,
            reason,
        };
        let expected_layout =
            super::aggregate_results::value_home_layout(declaration.structural_type, types)?;
        let expected_shape = expected_layout.shape();
        if !is_owned_parameter(declaration, types)
            || argument.access != StructuralAccess::Owned
            || destination.shape != expected_shape
        {
            return Err(invalid(
                "argument access or destination layout differs from its owned formal",
            ));
        }
        let (source, root_structural_type, source_byte_offset, index_runs) =
            if let Some(home) = live.structural_homes.get(&argument.place) {
                if home.access() != StructuralAccess::Owned {
                    return Err(invalid("source home is not owned"));
                }
                let (offset, indices) = if argument.path.is_empty() {
                    if home.structural_type() != declaration.structural_type
                        || home.layout != expected_layout
                    {
                        return Err(invalid(
                            "source home type or layout differs from its formal",
                        ));
                    }
                    if home.multiplicity() != declaration.multiplicity {
                        return Err(invalid("source home multiplicity differs from its formal"));
                    }
                    (0, Vec::new())
                } else {
                    let (selected, shape, offset, indices) =
                        runtime_projection(home.structural_type(), &argument.path, types)?;
                    if selected != declaration.structural_type || shape != expected_shape {
                        return Err(invalid(
                            "source home projection type or layout differs from its formal",
                        ));
                    }
                    (offset, indices)
                };
                let source = home.argument_source();
                (source, home.structural_type(), offset, indices)
            } else if let Some((establishment, structural_type)) =
                live.trivial_affine_locals.get(&argument.place)
            {
                if !argument.path.is_empty()
                    || *structural_type != declaration.structural_type
                    || declaration.multiplicity != StructuralMultiplicity::Affine
                    || expected_shape.byte_size != 0
                {
                    return Err(invalid("empty affine local does not match its formal"));
                }
                (
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: *establishment,
                    },
                    *structural_type,
                    0,
                    Vec::new(),
                )
            } else {
                let parameter = prepared
                    .parameter(argument.place)
                    .ok_or_else(|| invalid("source has no home or ingress parameter"))?;
                if parameter.access != StructuralAccess::Owned {
                    return Err(invalid("source parameter is not owned"));
                }
                let (offset, indices) = if argument.path.is_empty() {
                    if parameter.multiplicity != declaration.multiplicity
                        || parameter.structural_type != declaration.structural_type
                        || parameter.shape != expected_shape
                    {
                        return Err(invalid(
                            "source parameter custody, type or layout differs from its formal",
                        ));
                    }
                    (0, Vec::new())
                } else {
                    let (selected, shape, offset, indices) =
                        runtime_projection(parameter.structural_type, &argument.path, types)?;
                    if selected != declaration.structural_type || shape != expected_shape {
                        return Err(invalid(
                            "source parameter projection type or layout differs from its formal",
                        ));
                    }
                    (offset, indices)
                };
                (
                    parameter.placement.clone().into(),
                    parameter.structural_type,
                    offset,
                    indices,
                )
            };
        Ok(TargetStructuralArgument {
            place: argument.place,
            access: argument.access,
            path: argument.path.clone(),
            root_structural_type,
            structural_type: declaration.structural_type,
            shape: expected_shape,
            source_byte_offset,
            fixed_array_length: None,
            element_stride: None,
            indices: super::scalar_sources::runtime_indices(
                index_runs,
                function,
                &super::scalar_sources::ScalarSources::from(live),
            )?,
            source,
            destination: destination.placement.clone(),
        })
    }
}

pub(crate) mod selected_boundary_calls {
    use crate::LoweringError;
    use crate::selected_boundary_calls::SelectedBoundaryCall;
    use abstract_operations::{
        AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
    };
    use semantic_vocabulary::MachineId;
    use std::collections::BTreeMap;
    use target_operations::TargetUnitOperation;

    pub(crate) fn resolve(
        operation: &AbstractOperation,
        installed: &SelectedBoundaryCall,
        functions: &BTreeMap<MachineId, &AbstractFunction>,
    ) -> Result<(AbstractOperation, target_operations::NativeCallOrigin), LoweringError> {
        let AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources: _,
            completion_receipts,
        } = operation
        else {
            return Err(LoweringError::SelectedBoundaryCallEvidenceMismatch {
                machine: installed.caller,
                operation: installed.invocation_site,
                boundary: installed.boundary,
            });
        };
        let invalid = || LoweringError::SelectedBoundaryCallShapeMismatch {
            machine: installed.caller,
            operation: *psi_operation,
            boundary: *boundary,
        };
        if result != &installed.result {
            return Err(invalid());
        }
        let callee = installed.callee;
        let candidate = functions.get(&callee).copied().ok_or_else(invalid)?;
        let claim_transfers = completion_receipts
            .iter()
            .map(|receipt| terminal_psi::ClaimTransfer {
                claim: receipt.claim,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
            .collect::<Vec<_>>();
        let resolved = match (result, &candidate.result) {
            (abstract_operations::AbstractBoundaryResult::Unit, AbstractFunctionResult::Unit) => {
                AbstractOperation::CallStructural {
                    psi_operation: *psi_operation,
                    callee: callee.into(),
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    result: AbstractStructuralCallResult::Unit,
                }
            }
            (
                abstract_operations::AbstractBoundaryResult::Structural(result),
                AbstractFunctionResult::Structural(candidate_result),
            ) if result.structural_type == candidate_result.structural_type
                && result.multiplicity == candidate_result.multiplicity =>
            {
                AbstractOperation::CallStructural {
                    psi_operation: *psi_operation,
                    result: AbstractStructuralCallResult::Structural {
                        result: result.clone(),
                        selected_evidence: Vec::new(),
                    },
                    callee: callee.into(),
                    arguments: arguments.clone(),
                    structural_arguments: structural_arguments.clone(),
                    claim_transfers,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            }
            (
                abstract_operations::AbstractBoundaryResult::Scalar(result),
                AbstractFunctionResult::Scalar(candidate_result),
            ) if result.scalar_type == candidate_result.scalar_type => {
                if structural_arguments.is_empty() && claim_transfers.is_empty() {
                    AbstractOperation::Call {
                        psi_operation: *psi_operation,
                        result: result.value,
                        scalar_type: result.scalar_type,
                        callee,
                        arguments: arguments.clone(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    }
                } else {
                    AbstractOperation::CallStructural {
                        psi_operation: *psi_operation,
                        result: AbstractStructuralCallResult::Scalar(*result),
                        callee: callee.into(),
                        arguments: arguments.clone(),
                        structural_arguments: structural_arguments.clone(),
                        claim_transfers,
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    }
                }
            }
            _ => return Err(invalid()),
        };
        let origin = installed.origin.clone();
        Ok((resolved, origin))
    }

    pub(crate) fn retain_origin(
        operations: &mut [TargetUnitOperation],
        installed: &SelectedBoundaryCall,
        origin: target_operations::NativeCallOrigin,
    ) -> Result<(), LoweringError> {
        let [
            TargetUnitOperation::Call {
                psi_operation,
                target,
                origin: actual,
                ..
            },
        ] = operations
        else {
            return Err(LoweringError::SelectedBoundaryCallShapeMismatch {
                machine: installed.caller,
                operation: installed.invocation_site.operation().ok_or(
                    LoweringError::BoundaryRealizationMismatch(installed.boundary),
                )?,
                boundary: installed.boundary,
            });
        };
        if semantic_vocabulary::InvocationSite::Operation(*psi_operation)
            != installed.invocation_site
            || *target != target_operations::NativeCallTarget::Machine(installed.callee)
            || *actual != target_operations::NativeCallOrigin::Authored
        {
            return Err(LoweringError::SelectedBoundaryCallEvidenceMismatch {
                machine: installed.caller,
                operation: installed.invocation_site,
                boundary: installed.boundary,
            });
        }
        *actual = origin;
        Ok(())
    }
}
