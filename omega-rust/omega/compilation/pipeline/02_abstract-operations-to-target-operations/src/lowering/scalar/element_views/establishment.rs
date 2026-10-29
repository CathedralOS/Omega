//! Establish descriptors over the actual backing coordinate. Pointer regions
//! remain ordered: flattening a Referent into an inline offset would construct
//! a view over descriptor storage instead of the selected referent.
use super::*;

pub(super) fn source(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: ElementViewBackings<'_>,
    source: &StructuralArgument,
    element: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> Result<
    (
        StructuralTypeId,
        u32,
        Vec<target_operations::TargetStructuralRuntimeIndex>,
        target_operations::TargetViewExtent,
        u32,
        TargetStructuralArgumentSource,
    ),
    LoweringError,
> {
    let invalid = || {
        super::descriptor_operands::refusal(
            function,
            &(
                source,
                element,
                region,
                function.structural_parameter(source.place),
                backings.homes.get(&source.place),
                backings.address_joins.contains(&source.place),
            ),
        )
    };
    // A `&[u8]` view over `collection[a..b]` ends its projection in a
    // `FixedByteRange` window: the field path still names the fixed byte
    // array and the window contributes the descriptor's own byte offset and
    // extent, leaving the backing stride at the `u8` element's.
    let (field_path, byte_window) = match source.path.last() {
        Some(StructuralPathSegment::FixedByteRange { start, end }) => {
            (&source.path[..source.path.len() - 1], Some((*start, *end)))
        }
        _ => (&source.path[..], None),
    };
    if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!()) {
        return Err(invalid());
    }
    // Declared domains constrain contents, not the backing layout. Source
    // admission retains membership and loan obligations separately.
    let (root_structural_type, backing) =
        if let Some(semantic) = function.structural_parameter(source.place) {
            if !matches!(
                semantic.access,
                StructuralAccess::SharedBorrow | terminal_psi::mutating_access_pair_pattern!()
            ) || (source.access == StructuralAccess::MutableBorrow
                && !matches!(
                    semantic.access,
                    terminal_psi::mutating_access_pair_pattern!()
                ))
                || semantic.multiplicity != StructuralMultiplicity::Unrestricted
            {
                return Err(invalid());
            }
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.place == source.place)
                .ok_or_else(invalid)?;
            if parameter.structural_type != semantic.structural_type {
                return Err(invalid());
            }
            (
                semantic.structural_type,
                TargetStructuralArgumentSource::from(parameter.placement.clone()),
            )
        } else if let Some(backing) = live_aggregate_backing(backings.homes, source.place) {
            backing
        } else {
            backings
                .descriptor(function, structural_types, source)
                .or_else(|| backings.address(function, structural_types, source))
                .ok_or_else(invalid)?
        };
    let mut shape_cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let regions = crate::lowering::structural_layout::reference_projection_regions(
        root_structural_type,
        field_path,
        structural_types,
    )
    .filter(|regions| regions.readable)
    .ok_or_else(invalid)?;
    let (projected_storage, projected_shape, source_byte_offset, runtime_elements) =
        runtime_storage_projection(
            regions.root,
            regions.path,
            structural_types,
            &mut shape_cache,
            &mut active,
        )?;
    let indices = crate::lowering::control_flow::scalar_sources::runtime_indices(
        regions.selector_runs(runtime_elements),
        function,
        &backings.selectors,
    )?;
    let collection_shape = projected_storage
        .shape(structural_types)
        .ok_or_else(invalid)?;
    let geometry = terminal_semantics::element_view_shape_region_geometry(
        structural_types.values().copied(),
        &collection_shape,
        element,
        region,
        source.access,
    )
    .ok_or_else(invalid)?;
    if region == terminal_psi::ElementViewRegion::WholeCollection
        && let StructuralTypeShape::FixedArray {
            element: original, ..
        } = &*collection_shape
        && *original != element
    {
        let relation = terminal_semantics::element_representation_relation(
            structural_types.values().copied(),
            *original,
            element,
            source.access,
        )
        .ok_or_else(invalid)?;
        for (identity, layout) in [
            (*original, &relation.source_layout),
            (element, &relation.target_layout),
        ] {
            let shape = crate::lowering::structural_layout::structural_shape(
                identity,
                structural_types,
                &mut shape_cache,
                &mut active,
            )?;
            if u64::from(shape.byte_size) != layout.size
                || u64::from(shape.alignment) != layout.alignment
                || layout.leaves.iter().any(|leaf| {
                    scalar_leaf_offset(structural_types, identity, &leaf.path, leaf.scalar_type)
                        .map(u64::from)
                        != Some(leaf.offset)
                })
            {
                return Err(invalid());
            }
        }
    }
    let (extent_kind, storage, representation) = match geometry {
        terminal_semantics::ElementViewRegionGeometry::WholeCollection(collection) => {
            (collection.extent, collection.storage, None)
        }
        terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
            byte_extent,
            element: layout,
        } => {
            if byte_window.is_some() {
                return Err(invalid());
            }
            let storage = if matches!(&*collection_shape, StructuralTypeShape::ByteSequence(_)) {
                terminal_semantics::CollectionStorage::Borrowed
            } else {
                terminal_semantics::CollectionStorage::Inline
            };
            (
                terminal_semantics::CollectionExtent::Fixed(byte_extent),
                storage,
                Some(layout),
            )
        }
    };
    let element_shape = crate::lowering::structural_layout::structural_shape(
        element,
        structural_types,
        &mut shape_cache,
        &mut active,
    )?;
    let element_stride = crate::lowering::structural_layout::checked_align_up_u32(
        element_shape.byte_size,
        u32::from(element_shape.alignment),
    )
    .ok_or_else(invalid)?;
    if element_stride == 0
        || match storage {
            terminal_semantics::CollectionStorage::Inline => {
                (if representation.is_some() {
                    match extent_kind {
                        terminal_semantics::CollectionExtent::Fixed(length) => Some(length),
                        _ => None,
                    }
                } else {
                    match extent_kind {
                        terminal_semantics::CollectionExtent::Fixed(length) => {
                            u64::from(element_stride).checked_mul(length)
                        }
                        // An owned byte carrier's inline elements follow its
                        // live-length word; capacity bounds their span.
                        terminal_semantics::CollectionExtent::LiveLength { capacity } => {
                            u64::from(element_stride)
                                .checked_mul(capacity)
                                .and_then(|span| span.checked_add(8))
                        }
                        terminal_semantics::CollectionExtent::StoredLength => None,
                    }
                }) != Some(u64::from(projected_shape.byte_size))
            }
            // The carrier contains one pointer, not capacity bytes. Dynamic
            // narrowing is a following subslice over the established descriptor.
            terminal_semantics::CollectionStorage::Borrowed => {
                let carrier_size = match extent_kind {
                    terminal_semantics::CollectionExtent::StoredLength => 16,
                    terminal_semantics::CollectionExtent::Fixed(_) => 8,
                    terminal_semantics::CollectionExtent::LiveLength { .. } => {
                        return Err(invalid());
                    }
                };
                projected_shape.byte_size != carrier_size || byte_window.is_some()
            }
        }
    {
        return Err(invalid());
    }
    if let Some(layout) = representation {
        if layout.size != u64::from(element_stride)
            || layout.alignment != u64::from(element_shape.alignment)
            || layout.leaves.iter().any(|leaf| {
                scalar_leaf_offset(structural_types, element, &leaf.path, leaf.scalar_type)
                    .map(u64::from)
                    != Some(leaf.offset)
            })
        {
            return Err(invalid());
        }
    }
    let (window_offset, extent) = match (byte_window, extent_kind) {
        (Some((start, end)), terminal_semantics::CollectionExtent::Fixed(length))
            if start <= end && end <= length =>
        {
            (
                u32::try_from(
                    start
                        .checked_mul(u64::from(element_stride))
                        .ok_or_else(invalid)?,
                )
                .map_err(|_| invalid())?,
                target_operations::TargetViewExtent::Fixed(end - start),
            )
        }
        // A static window cannot bound a live count; dynamic narrowing is a
        // following subslice over the established descriptor.
        (Some(_), _) => return Err(invalid()),
        (None, terminal_semantics::CollectionExtent::Fixed(length)) => {
            (0, target_operations::TargetViewExtent::Fixed(length))
        }
        // An owned byte carrier's viewed elements start eight bytes into its
        // storage, after the live-length word.
        (None, terminal_semantics::CollectionExtent::LiveLength { .. }) => {
            (8, target_operations::TargetViewExtent::LiveLength)
        }
        (None, terminal_semantics::CollectionExtent::StoredLength) => {
            (0, target_operations::TargetViewExtent::StoredLength)
        }
    };
    let source_byte_offset = source_byte_offset
        .checked_add(window_offset)
        .ok_or_else(invalid)?;
    Ok((
        root_structural_type,
        source_byte_offset,
        indices,
        extent,
        element_stride,
        backing,
    ))
}
