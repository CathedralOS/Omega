//! Borrowed call arguments resolved against live graph storage: primitive
//! referents, projected loans, address joins, and byte or element views.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::PreparedFunctionSignature;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::MachineId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralParameter,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

/// Resolve borrowed storage or an explicit reference crossing independently
/// of the call result.
pub(super) fn argument(
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
    // A live aggregate home lends one exact subtree through a borrow: the
    // producer keeps whole storage, and the argument transports the leaf's
    // address inside it. Claims stay on the owner, independently of the
    // borrowed argument's unrestricted multiplicity. An empty path borrows
    // the home whole when the formal is no primitive reference — primitive
    // storage keeps its own custody channel below.
    if let Some(home) = live.structural_homes.get(&argument.place)
        && (!argument.path.is_empty()
            || !super::primitive_storage::is_primitive_reference(declaration, types))
    {
        // A selected owner has a block-arrival home, not one distinguished
        // incoming producer. Borrow that destination's storage; choosing an
        // incoming operation would lose the runtime selection and its updates.
        let source = crate::lowering::unit::argument_roots::resolve(
            argument.place,
            function,
            prepared.parameters.iter(),
            &live.structural_homes,
            &live.address_joins,
            &live.block_views,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
        .source;
        let mut shape_cache = BTreeMap::new();
        let mut active = BTreeSet::new();
        let (projected_type, projected_shape, source_byte_offset, runs) =
            if argument.path.is_empty() {
                (
                    home.structural_type(),
                    crate::lowering::structural_layout::structural_shape(
                        home.structural_type(),
                        types,
                        &mut shape_cache,
                        &mut active,
                    )?,
                    0,
                    Vec::new(),
                )
            } else {
                crate::lowering::structural_layout::runtime_borrowed_projection(
                    home.structural_type(),
                    &argument.path,
                    argument.access,
                    Some(declaration.structural_type),
                    types,
                    &mut shape_cache,
                    &mut active,
                )?
            };
        // Only an inline projection remains inside the aggregate's storage.
        // A reference or view crossing selects a separate live backing region;
        // its pointer and selector bounds belong to the canonical path walk.
        let inline = crate::lowering::structural_layout::reference_projection_regions(
            home.structural_type(),
            &argument.path,
            types,
        )
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
        .pointer_slots
        .is_empty();
        // Psi checks the formal's memberships against this exact occurrence.
        // Borrowing retains the owner's storage and claims; domain rows and
        // establishment receipts add no address or ABI words. The same layout
        // join therefore serves every carrier, not only bounded byte owners.
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
            || crate::lowering::structural_layout::internal_parameter_shape(
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
    if !terminal_semantics::borrowed_referent_matches(
        types.values().copied(),
        identity,
        declaration.structural_type,
    ) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(TargetStructuralArgument {
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

/// Invocation and joined storage lend from their existing address, not a copy.
/// Both use the same typed projection and dominating scalar operands; only the
/// source placement differs. Runtime selectors retain their verified paths.
/// Access cannot widen, and the projected referent must match the callee.
/// Unit call admission owns qualification discharge and writable invariants;
/// a qualified loan has the same address and layout as its carrier.
pub(super) fn storage_argument(
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
    let root = crate::lowering::unit::argument_roots::resolve(
        argument.place,
        function,
        prepared.parameters.iter(),
        &live.structural_homes,
        &live.address_joins,
        &live.block_views,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let root_type = root.declaration.structural_type;
    let access = root.declaration.access;
    let source = root.source;
    let allowed = match access {
        StructuralAccess::MutableBorrow => argument.access != StructuralAccess::Owned,
        StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow => argument.access == StructuralAccess::WriteOnlyBorrow,
        // A live by-value arrival owns its completed storage. Borrow its
        // original home for this call; do not reinterpret its ABI fragments
        // as the pointer parameter or manufacture a copied referent.
        StructuralAccess::Owned => live.owned_arrivals.contains(&argument.place),
    };
    let (selected, referent, offset, runs) =
        crate::lowering::structural_layout::runtime_borrowed_projection(
            root_type,
            &argument.path,
            argument.access,
            Some(declaration.structural_type),
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
    if !allowed
        || selected != declaration.structural_type
        || destination.shape
            != crate::lowering::structural_layout::internal_parameter_shape(
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

/// An available address join lends a fixed-array window of its joined
/// referent exactly as an incoming parameter's field window does: the backing
/// keeps its real type and field path, the carried address roots it, and the
/// presented length grants neither storage nor resize.
pub(super) fn address_join_window_argument(
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
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let root = crate::lowering::unit::argument_roots::resolve(
        argument.place,
        function,
        std::iter::empty::<&TargetStructuralParameter>(),
        &live.structural_homes,
        &live.address_joins,
        &live.block_views,
        types,
        &mut shape_cache,
        &mut active,
    )?;
    if let Some(window) = crate::lowering::unit::roster_arguments::fixed_array_view_argument(
        argument,
        declaration,
        destination.shape,
        &destination.placement,
        callee,
        &root.declaration,
        root.backing_shape.byte_size,
        root.source.clone(),
        types,
        &mut shape_cache,
        &mut active,
    )? {
        return Ok(window);
    }
    // A record field can already be the element-view or byte-view descriptor.
    // The address join owns the record; the call lends that field at its
    // offset. This is not a fixed-array window and not a new descriptor.
    stored_descriptor_field(argument, declaration, destination, &root, function, types)
}

/// A stored descriptor may be lent at its own access, or read shared when
/// the stored ceiling is mutable. Canonical whole descriptors omit the
/// ceiling (`None`) and follow the argument access.
fn view_access_lends(
    stored: Option<StructuralAccess>,
    presented: Option<StructuralAccess>,
    argument: StructuralAccess,
) -> bool {
    let stored_allows = match stored {
        None => true,
        Some(StructuralAccess::MutableBorrow) => {
            matches!(
                argument,
                StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
            )
        }
        Some(access) => access == argument,
    };
    let presented_allows = match presented {
        None => true,
        Some(access) => access == argument,
    };
    stored_allows && presented_allows
}

/// Lend a descriptor stored in a field of an address-joined record.
///
/// The field's structural type is the callee's view. Shared access may read
/// a mutable descriptor; the loan does not retarget or resize it.
fn stored_descriptor_field(
    argument: &terminal_psi::StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    root: &crate::lowering::unit::argument_roots::StructuralArgumentRoot,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    if argument.path.is_empty()
        || !argument
            .path
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(unsupported());
    }
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let (selected, shape, offset, runs) =
        crate::lowering::structural_layout::runtime_storage_projection(
            root.declaration.structural_type,
            &argument.path,
            types,
            &mut shape_cache,
            &mut active,
        )?;
    if !runs.is_empty() {
        return Err(unsupported());
    }
    let field_type = selected.declaration(
        root.declaration.structural_type,
        types,
        terminal_psi::StructuralFieldType::canonical_leaf_type,
    )?;
    let same_view = match (
        types.get(&field_type).map(|row| &row.shape),
        types
            .get(&declaration.structural_type)
            .map(|row| &row.shape),
    ) {
        (
            Some(terminal_psi::StructuralTypeShape::ElementView {
                element: stored,
                access: stored_access,
            }),
            Some(terminal_psi::StructuralTypeShape::ElementView {
                element: presented,
                access: presented_access,
            }),
        ) => {
            stored == presented
                && view_access_lends(*stored_access, *presented_access, argument.access)
        }
        (
            Some(terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: stored_access,
                },
            )),
            Some(terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: presented_access,
                },
            )),
        ) => view_access_lends(*stored_access, *presented_access, argument.access),
        _ => false,
    };
    let end = u64::from(offset).checked_add(u64::from(shape.byte_size));
    if !same_view
        || destination.shape != ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
        || end.is_none_or(|end| end > u64::from(root.backing_shape.byte_size))
    {
        return Err(unsupported());
    }
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
        indices: Vec::new(),
        source: root.source.clone(),
        destination: destination.placement.clone(),
    })
}

/// Borrowed byte views keep their exact descriptor source through ordinary
/// calls: an established literal or subslice producer, a borrowed block
/// parameter, or the caller's own machine parameter. Shared views may arrive
/// from all three. Element-view producers also retain exclusive access through
/// the same source-chain resolver used by indexed writes. The root's own access
/// authorizes the argument, so affine custody cannot promote a shared source.
/// Fixed arrays instead present their backing: a parameter's field window or
/// a dominating local array window, shared or exclusive.
#[allow(clippy::too_many_arguments)]
pub(super) fn byte_argument(
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
    // A fixed array lends its original backing through the same call-scoped
    // presentation the structural-result lane emits: an incoming parameter's
    // field window keeps its real path, and a dominating local array lends its
    // own home. A scalar or unit result changes no custody. Resolve local
    // backing before a projected parameter lookup: geometry does not determine
    // whether the source storage is an incoming parameter or a local result.
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    // Resolve every available backing through the same origin join. A
    // block-owned array lends its selected home just as an operation result
    // or machine arrival does; none is a borrowed descriptor by residence.
    if prepared.parameter(argument.place).is_some()
        || live.structural_homes.contains_key(&argument.place)
        || live.block_views.contains(&argument.place)
        || live.address_joins.contains(&argument.place)
    {
        let root = crate::lowering::unit::argument_roots::resolve(
            argument.place,
            function,
            prepared.parameters.iter(),
            &live.structural_homes,
            &live.address_joins,
            &live.block_views,
            types,
            &mut shape_cache,
            &mut active,
        )?;
        if let Some(presentation) =
            crate::lowering::unit::roster_arguments::fixed_array_view_argument(
                argument,
                declaration,
                destination.shape,
                &destination.placement,
                callee,
                &root.declaration,
                root.backing_shape.byte_size,
                root.source,
                types,
                &mut shape_cache,
                &mut active,
            )?
        {
            return Ok(presentation);
        }
    }
    if !argument.path.is_empty() {
        // Fixed-array presentations above manufacture a call-local descriptor.
        // Every other projection lends the descriptor already stored at that
        // coordinate, using the same home and geometry as ordinary loans.
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
    // A descriptor view carries pointer and extent; a bounded view over a
    // fixed array (`&[u8; N]`) carries only the pointer, its extent static.
    let carrier_size = match types
        .get(&declaration.structural_type)
        .map(|row| &row.shape)
    {
        Some(terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedView { .. },
        )) => 8,
        _ => 16,
    };
    if argument.access != declaration.access
        || (!crate::lowering::scalar::byte_views::is_byte_parameter(declaration, types)
            && !crate::lowering::scalar::element_views::is_element_view_parameter(
                declaration,
                types,
            ))
        || destination.shape != ValueShape::borrowed_reference(carrier_size, 8)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (identity, source) = if let Some((producer, result)) =
        crate::lowering::scalar::byte_views::descriptor_result(
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
        let source = if crate::lowering::scalar::element_views::is_element_view_parameter(
            declaration,
            types,
        ) {
            let values = super::observations::scalar_values(live, &prepared.scalar_parameters)?;
            let (_, identity, access) =
                crate::lowering::scalar::element_views::resolve_element_view(
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
            let values = super::observations::scalar_values(live, &prepared.scalar_parameters)?;
            let (_, access) = crate::lowering::scalar::byte_views::resolve_byte_view(
                function,
                types,
                &prepared.parameters,
                live.element_view_backings(),
                &values,
                &live.lengths,
                argument.place,
                &mut Vec::new(),
            )?;
            if access != argument.access && !(access == StructuralAccess::MutableBorrow && shared) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: *producer,
            }
        };
        (*structural_type, source)
    } else if live.block_views.contains(&argument.place) {
        let (entry, parameter) = function
            .block_entries
            .iter()
            .find_map(|entry| {
                entry
                    .structural_parameter(argument.place)
                    .map(|parameter| (entry, parameter))
            })
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        // The root's own access authorizes the argument, exactly as an
        // incoming machine parameter does: an exclusive block parameter
        // lends shared, exclusive or write-only, a shared one only shared.
        let allowed = match parameter.access {
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
    if !terminal_semantics::borrowed_referent_matches(
        types.values().copied(),
        identity,
        declaration.structural_type,
    ) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(TargetStructuralArgument {
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
