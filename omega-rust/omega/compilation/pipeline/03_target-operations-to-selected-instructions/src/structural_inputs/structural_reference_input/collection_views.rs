//! Collection descriptors select real storage across retained pointer regions.
//! The final region owns collection geometry; preceding regions own descriptor
//! loads and their selector ranges. Access remains a separate admission check.
use super::*;

/// Final-region collection geometry plus ordered pointer crossings. Clients
/// retaining only inline geometry keep using collection_projection instead.
pub(crate) struct ProjectedCollection<'types> {
    pub shape: std::borrow::Cow<'types, StructuralTypeShape>,
    pub offset: u32,
    pub elements: Vec<RuntimeElement>,
    pub pointer_slots: Vec<PointerSlot>,
    pub byte_size: u32,
}

pub(crate) fn projected_collection<'types>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &'types [StructuralTypeDeclaration],
) -> Option<ProjectedCollection<'types>> {
    let regions = storage_projection_regions(root, path, declarations)?;
    if !regions.readable {
        return None;
    }
    let leaf_path = &path[regions.segment_start..];
    let (shape, offset, leaf_elements) =
        collection_projection(regions.root, leaf_path, declarations)?;
    let (storage, _, _) = runtime_storage_projection(regions.root, leaf_path, declarations)?;
    let byte_size = field_shape(&storage, declarations, &mut Vec::new())?.byte_size;
    let mut elements = regions.elements;
    elements.extend(leaf_elements);
    Some(ProjectedCollection {
        shape,
        offset,
        elements,
        pointer_slots: regions.pointer_slots,
        byte_size,
    })
}

pub(crate) struct CollectionViewGeometry {
    pub offset: u32,
    pub elements: Vec<RuntimeElement>,
    pub pointer_slots: Vec<PointerSlot>,
    pub extent: terminal_semantics::CollectionExtent,
    pub stride: u32,
    pub storage: terminal_semantics::CollectionStorage,
}

pub(crate) fn collection_view_geometry(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<CollectionViewGeometry> {
    collection_view_region_geometry(
        source,
        argument,
        view_type,
        declarations,
        terminal_psi::ElementViewRegion::WholeCollection,
    )
}
pub(crate) fn collection_view_region_geometry(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    region: terminal_psi::ElementViewRegion,
) -> Option<CollectionViewGeometry> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    if source.place != argument.place
        || !matches!(
            (source.access, argument.access),
            (
                StructuralAccess::MutableBorrow,
                terminal_psi::reading_borrow_pair_pattern!(),
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            ) | (
                StructuralAccess::Owned,
                StructuralAccess::SharedBorrow | terminal_psi::mutating_access_pair_pattern!(),
            )
        )
        || !(source.multiplicity == StructuralMultiplicity::Unrestricted
            || (source.multiplicity == StructuralMultiplicity::Affine
                && source.access == StructuralAccess::Owned
                && matches!(
                    argument.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                ))
            || (source.multiplicity == StructuralMultiplicity::Affine
                && matches!(
                    source.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )))
    {
        return None;
    }
    // A `&[u8]` view ends its path in a `FixedByteRange` window over the
    // fixed byte array the field prefix names; the window start is a byte
    // offset and its width replaces the array length in the descriptor.
    let (field_path, byte_window) = match argument.path.last() {
        Some(StructuralPathSegment::FixedByteRange { start, end }) => (
            &argument.path[..argument.path.len() - 1],
            Some((*start, *end)),
        ),
        _ => (&argument.path[..], None),
    };
    let view_element = match &find_structural_type(declarations, view_type)?.shape {
        StructuralTypeShape::ElementView {
            element: view_element,
            ..
        } => Some(*view_element),
        shape if shape.is_borrowed_byte_carrier() => None,
        _ => return None,
    };
    if !terminal_semantics::descriptor_source_projection_accessible(
        declarations.iter(),
        source.structural_type,
        source.multiplicity,
        source.access,
        field_path,
        argument.access,
    ) {
        return None;
    }
    let collection = projected_collection(source.structural_type, field_path, declarations)?;
    let collection_shape = collection.shape;
    let offset = collection.offset;
    let elements = collection.elements;
    let element = view_element.or_else(|| {
        declarations.iter().find_map(|declaration| {
            (declaration.scalar_restriction.is_none()
                && matches!(declaration.shape,
            StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
            if Ok(integer) == semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned, 8)))
            .then_some(declaration.id)
            .filter(|element| {
                terminal_semantics::element_view_shape_region_geometry(
                    declarations.iter(),
                    &collection_shape,
                    *element,
                    region,
                    argument.access,
                )
                .is_some()
            })
        })
    })?;
    let geometry = terminal_semantics::element_view_shape_region_geometry(
        declarations.iter(),
        &collection_shape,
        element,
        region,
        argument.access,
    )?;
    if region == terminal_psi::ElementViewRegion::WholeCollection
        && !typed_collection_layout_matches(
            &collection_shape,
            element,
            argument.access,
            declarations,
        )
    {
        return None;
    }
    let (backing, representation) = match geometry {
        terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing) => (backing, None),
        terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
            byte_extent,
            element: layout,
        } => {
            if byte_window.is_some() {
                return None;
            }
            let storage = if matches!(&*collection_shape, StructuralTypeShape::ByteSequence(_)) {
                terminal_semantics::CollectionStorage::Borrowed
            } else {
                terminal_semantics::CollectionStorage::Inline
            };
            (
                terminal_semantics::CollectionBacking {
                    extent: terminal_semantics::CollectionExtent::Fixed(byte_extent),
                    storage,
                },
                Some(layout),
            )
        }
    };
    if backing.storage == terminal_semantics::CollectionStorage::Borrowed
        && (byte_window.is_some()
            || (view_element.is_some()
                && representation.is_none()
                && backing.extent != terminal_semantics::CollectionExtent::StoredLength))
    {
        return None;
    }
    let extent = backing.extent;
    terminal_semantics::collection_view_descriptor(
        &find_structural_type(declarations, view_type)?.shape,
        Some(extent),
        region,
        &argument.path,
        find_structural_type(declarations, element),
    )?;
    let element_stride = align(
        shape(element, declarations)?.byte_size,
        shape(element, declarations)?.alignment,
    )?;
    if let Some(layout) = &representation {
        if layout.size != u64::from(element_stride)
            || layout.alignment != u64::from(shape(element, declarations)?.alignment)
            || layout.leaves.iter().any(|leaf| {
                element_leaf_geometry(element, &leaf.path, leaf.scalar_type, declarations)
                    .is_none_or(|(offset, _)| u64::from(offset) != leaf.offset)
            })
        {
            return None;
        }
    }
    let (window_offset, element_count, extent) = match (byte_window, extent) {
        (Some((start, end)), terminal_semantics::CollectionExtent::Fixed(length))
            if start <= end && end <= length =>
        {
            let byte_offset = start.checked_mul(u64::from(element_stride))?;
            (
                u32::try_from(byte_offset).ok()?,
                end - start,
                terminal_semantics::CollectionExtent::Fixed(end - start),
            )
        }
        // A static window cannot bound a live count; dynamic narrowing is a
        // following subslice over the established descriptor.
        (Some(_), _) => return None,
        // An owned byte carrier's live-length word precedes its elements, so
        // the viewed bytes start eight bytes into the collection's storage;
        // capacity bounds the span its live extent counts.
        (None, terminal_semantics::CollectionExtent::LiveLength { capacity }) => (
            8,
            capacity,
            terminal_semantics::CollectionExtent::LiveLength { capacity },
        ),
        (None, terminal_semantics::CollectionExtent::Fixed(length)) => (
            0,
            length,
            terminal_semantics::CollectionExtent::Fixed(length),
        ),
        // This count is loaded at runtime. Only descriptor storage contributes
        // to the finite footprint below, never a fabricated capacity.
        (None, terminal_semantics::CollectionExtent::StoredLength) => {
            (0, 0, terminal_semantics::CollectionExtent::StoredLength)
        }
    };
    let offset = offset.checked_add(window_offset)?;
    let storage_size = match backing.storage {
        terminal_semantics::CollectionStorage::Inline => {
            if representation.is_some() {
                element_count
            } else {
                element_count.checked_mul(u64::from(element_stride))?
            }
        }
        terminal_semantics::CollectionStorage::Borrowed => {
            if extent == terminal_semantics::CollectionExtent::StoredLength {
                16
            } else {
                8
            }
        }
    };
    // The selected collection owns this finite footprint. After indexing a
    // view, `offset` is in its element storage, not in the root descriptor's
    // two words. Projection already validates the containing field layout.
    let selected_size = collection.byte_size;
    let end = u64::from(window_offset).checked_add(storage_size)?;
    if end > u64::from(selected_size) {
        return None;
    }
    Some(CollectionViewGeometry {
        offset,
        elements,
        pointer_slots: collection.pointer_slots,
        extent,
        stride: element_stride,
        storage: backing.storage,
    })
}
