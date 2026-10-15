//! Input-only reconstruction of structural storage and borrowed-pointer geometry.
//! Whole owned aggregates and borrowed referents share recursive payload layout;
//! their callers independently check access, ownership and exact ABI placement.
//! Reference formation and calls retain selectors in ordered pointer regions.
//! A flattened selector roster authenticates bounds but cannot be reapplied
//! after the loads: that would index different storage. Scalar crossings retain
//! their address-and-codec descriptor, while aggregate crossings load a pointer.
use calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape};
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    find_structural_type, structural_case_position,
};

pub(crate) fn scalar_storage_type(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ScalarType> {
    let StructuralTypeShape::PrimitiveScalar(scalar) =
        find_structural_type(declarations, root)?.shape
    else {
        return None;
    };
    runtime_abi::ScalarStorageCodec::from_scalar(scalar).map(|_| scalar)
}

/// A thin boundary pointer discards the codec, so its entire logical footprint
/// must be valid for every backing the checked loan can denote. This is not
/// source authority: callers still retain the exact reference path and access.
pub(crate) fn scalar_reference_export_width(
    selected: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    declarations: &[StructuralTypeDeclaration],
) -> Option<u32> {
    let scalar = scalar_storage_type(selected, declarations)?;
    terminal_semantics::reference_storage_export_byte_width(scalar, access).map(u32::from)
}

/// Locate stack pointer bits after the caller has established pointer transport.
///
/// Internal reference plans retain a borrowed-reference class, while foreign
/// boundaries classify the same pointer bits as an integer word. The class is
/// not evidence of a loan: semantic access and the retained source/ABI join
/// establish that before this geometry query. Owned inline payloads must not
/// use this query to reinterpret their bytes as a pointer.
pub(crate) fn stack_pointer_offset(placement: &ValuePlacement) -> Option<u32> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            },
        ] => Some(*stack_byte_offset),
        [
            ValueLocation::Indirect {
                pointer:
                    IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment: 8,
                    },
                copy_stack_byte_offset: None,
                byte_size,
                alignment,
            },
        ] if *byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment => {
            Some(*stack_byte_offset)
        }
        _ => None,
    }
}

pub(crate) fn align(value: u32, alignment: u16) -> Option<u32> {
    let alignment = u32::from(alignment);
    Some(value.checked_add(alignment.checked_sub(1)?)? / alignment * alignment)
}

pub(crate) fn scalar_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::IeeeFloat(format) => Some(ValueShape::float(format.byte_size())),
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) if matches!(integer.bits(), 8 | 16 | 32 | 64) => {
            let bytes = integer.bits() / 8;
            Some(ValueShape::integer(u32::from(bytes), bytes))
        }
        _ => None,
    }
}

/// One runtime-selected element of a structural projection: the selector
/// value, the obligation the verified Terminal attached to the segment, the
/// array's element `stride`, and its declared `extent` — absent when the
/// carrier is an element view, whose extent is a live length observation
/// the obligation certifies, not a declaration the layout can reach for.
/// The address model scales `index` by `stride`; the obligation, not this
/// layout, is what proves `index < extent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeElement {
    pub(crate) index: semantic_vocabulary::ValueId,
    pub(crate) obligation: semantic_vocabulary::ObligationId,
    pub(crate) stride: u32,
    pub(crate) extent: Option<u64>,
}

/// Whether legalized runtime traversals are exactly a projection's runtime
/// elements, in path order: the same selector, obligation, stride and
/// extent. The accepted certificate beside each was joined by legalization.
pub(crate) fn runtime_indices_match(
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    elements: &[RuntimeElement],
) -> bool {
    indices.len() == elements.len()
        && indices.iter().zip(elements).all(|(index, element)| {
            index.operand.value == element.index
                && index.obligation == element.obligation
                && index.stride == element.stride
                && index.extent == element.extent
        })
}

/// The farthest a projection's runtime elements move its access past the
/// static offset: each element's last in-extent position times its stride.
/// This is layout geometry — every access the verified obligations admit
/// stays inside the root — not a bound on any selector.
enum RuntimeReach {
    /// No selector tuple can satisfy the retained element obligations.
    Empty,
    Last(u64),
}

fn runtime_reach(elements: &[RuntimeElement]) -> Option<RuntimeReach> {
    if elements.iter().any(|element| element.extent == Some(0)) {
        return Some(RuntimeReach::Empty);
    }
    elements
        .iter()
        .try_fold(0u64, |reach, element| {
            reach.checked_add(
                u64::from(element.stride).checked_mul(element.extent?.checked_sub(1)?)?,
            )
        })
        .map(RuntimeReach::Last)
}

/// The universal physical-span condition over admitted runtime selectors.
/// An empty domain changes only this condition, not root/path/type formation
/// or the selector certificates every caller independently joins.
fn runtime_span_within_storage(
    offset: u64,
    bytes: u64,
    elements: &[RuntimeElement],
    storage_bytes: u64,
) -> bool {
    match runtime_reach(elements) {
        Some(RuntimeReach::Empty) => true,
        Some(RuntimeReach::Last(reach)) => offset
            .checked_add(reach)
            .and_then(|end| end.checked_add(bytes))
            .is_some_and(|end| end <= storage_bytes),
        None => false,
    }
}

/// One pointer word selected within its own contiguous carrier region.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PointerSlot {
    pub(crate) byte_offset: u32,
    pub(crate) indices: std::ops::Range<usize>,
    pub(crate) scalar_descriptor: bool,
}

/// Address formation regions retain selectors on the side of each pointer
/// crossing where they were authored. Construction scratch, not another IR.
pub(crate) struct ReferenceSourceGeometry<Endpoint = StructuralTypeId> {
    /// The final contiguous storage region, distinct from the descriptor root.
    pub(crate) region_root: StructuralTypeId,
    /// Declared read permission at every crossing, not a liveness judgment.
    pub(crate) readable: bool,
    pub(crate) selected: Endpoint,
    pub(crate) byte_offset: u32,
    pub(crate) pointer_slots: Vec<PointerSlot>,
    pub(crate) elements: Vec<RuntimeElement>,
}

/// The extent word of the selected element descriptor, in element units.
/// Stored-reference crossings keep their own selector regions; selecting a
/// sibling descriptor never replaces this operand's path or captured indices.
pub(crate) fn element_view_length_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    let mut geometry = reference_source_geometry(root, path, declarations)?;
    if !geometry.readable
        || !matches!(
            find_structural_type(declarations, geometry.selected)?.shape,
            StructuralTypeShape::ElementView { .. }
        )
    {
        return None;
    }
    geometry.byte_offset = geometry.byte_offset.checked_add(8)?;
    Some(geometry)
}

pub(crate) fn descriptor_source_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    reference_source_geometry_with_leaf_type(root, path, declarations, descriptor_leaf_type)
}

/// Address formation retains the actual storage endpoint. An inline qualified
/// byte field has no nominal declaration, and equal-capacity owners elsewhere
/// in the catalog cannot supply its identity. The caller separately joins this
/// storage contract to the explicitly declared reference presentation.
pub(crate) fn reference_formation_geometry<'types>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &'types [StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry<std::borrow::Cow<'types, StructuralFieldType>>> {
    reference_geometry(root, path, declarations, |root, path| {
        runtime_storage_projection(root, path, declarations)
    })
}

// Pointer crossings select descriptor storage independently of the eventual
// scalar or aggregate endpoint. Inline fields use the same carrier as roots;
// their access remains on the original path, not on this layout declaration.
fn descriptor_leaf_type(field: &StructuralFieldType) -> Option<StructuralFieldType> {
    field
        .byte_sequence_carrier()
        .map(|carrier| {
            StructuralFieldType::ByteSequence(match carrier {
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => {
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
                }
                terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } => {
                    terminal_psi::ByteSequenceCarrier::BoundedView {
                        access: None,
                        capacity,
                    }
                }
                owned => owned,
            })
        })
        .or_else(|| field.canonical_leaf_type())
}

pub(crate) fn reference_source_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    reference_source_geometry_with_leaf_type(
        root,
        path,
        declarations,
        StructuralFieldType::canonical_leaf_type,
    )
}

/// Snapshot endpoint geometry retains byte-field copy shapes while sharing
/// the ordered pointer regions used by ordinary reference observations.
pub(crate) fn leaf_copy_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    reference_source_geometry_with_leaf_type(
        root,
        path,
        declarations,
        StructuralFieldType::leaf_copy_type,
    )
}

fn reference_source_geometry_with_leaf_type(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
    leaf_type: fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Option<ReferenceSourceGeometry> {
    reference_geometry(root, path, declarations, |root, path| {
        runtime_projection_with_leaf_type(root, path, declarations, leaf_type)
    })
}

/// Formation and call preparation share the independently bounded pointer
/// regions. Only the final leaf contract differs: a scalar call carries its
/// address and codec descriptor rather than loading the scalar value.
fn reference_geometry<Endpoint>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
    endpoint: impl Fn(
        StructuralTypeId,
        &[StructuralPathSegment],
    ) -> Option<(Endpoint, u32, Vec<RuntimeElement>)>,
) -> Option<ReferenceSourceGeometry<Endpoint>> {
    let regions = storage_projection_regions(root, path, declarations)?;
    let (selected, byte_offset, leaf_elements) =
        endpoint(regions.root, &path[regions.segment_start..])?;
    let mut elements = regions.elements;
    elements.extend(leaf_elements);
    Some(ReferenceSourceGeometry {
        region_root: regions.root,
        readable: regions.readable,
        selected,
        byte_offset,
        pointer_slots: regions.pointer_slots,
        elements,
    })
}

struct StorageProjectionRegions {
    root: StructuralTypeId,
    segment_start: usize,
    elements: Vec<RuntimeElement>,
    pointer_slots: Vec<PointerSlot>,
    readable: bool,
    uses_borrowed_backing: bool,
}

/// A stored reference and an indexed borrowed view both cross a pointer word. Keep
/// each selector on the side of the load where it addresses real storage.
fn storage_projection_regions(
    mut root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<StorageProjectionRegions> {
    let mut segment_start = 0;
    let mut elements = Vec::new();
    let mut pointer_slots = Vec::new();
    let mut readable = true;
    let mut uses_borrowed_backing = find_structural_type(declarations, root)?
        .shape
        .is_borrowed_byte_carrier();
    for (position, segment) in path.iter().enumerate() {
        if !matches!(
            segment,
            StructuralPathSegment::Referent
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
        ) {
            continue;
        }
        let selected = runtime_projection_with_leaf_type(
            root,
            &path[segment_start..position],
            declarations,
            descriptor_leaf_type,
        );
        let Some((carrier, byte_offset, region_elements)) = selected else {
            if *segment == StructuralPathSegment::Referent {
                return None;
            }
            continue;
        };
        let carrier_shape = &find_structural_type(declarations, carrier)?.shape;
        // Byte-view indexing uses the descriptor's backing even when native
        // address formation does not split it into another pointer region.
        // Preserve this distinction for origin inventory, including a byte
        // view selected below an otherwise inline record or array.
        uses_borrowed_backing |= carrier_shape.is_borrowed_byte_carrier();
        let (next_root, next_start, scalar_descriptor, descriptor_size) =
            match (segment, carrier_shape) {
                (
                    StructuralPathSegment::Referent,
                    StructuralTypeShape::Reference { referent, access },
                ) => {
                    readable &= matches!(
                        access,
                        terminal_psi::StructuralAccess::SharedBorrow
                            | terminal_psi::StructuralAccess::MutableBorrow
                    );
                    let scalar_descriptor = scalar_storage_type(*referent, declarations).is_some();
                    (
                        *referent,
                        position + 1,
                        scalar_descriptor,
                        if scalar_descriptor { 16 } else { 8 },
                    )
                }
                (
                    StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. },
                    StructuralTypeShape::ElementView { .. },
                ) => (carrier, position, false, 16),
                (
                    StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::RuntimeIndex { .. },
                    borrowed,
                ) if borrowed.is_borrowed_byte_carrier() => {
                    // Bounded byte views carry one pointer; unbounded views
                    // also carry a length. Both load the same backing word.
                    (
                        carrier,
                        position,
                        false,
                        shape(carrier, declarations)?.byte_size,
                    )
                }
                (StructuralPathSegment::Referent, _) => return None,
                _ => continue,
            };
        if shape(carrier, declarations)? != ValueShape::integer(descriptor_size, 8)
            || !scalar_access_within_storage(
                root,
                byte_offset,
                descriptor_size,
                &region_elements,
                declarations,
            )
        {
            return None;
        }
        let first_index = elements.len();
        elements.extend(region_elements);
        pointer_slots.push(PointerSlot {
            byte_offset,
            indices: first_index..elements.len(),
            scalar_descriptor,
        });
        root = next_root;
        segment_start = next_start;
    }
    Some(StorageProjectionRegions {
        root,
        segment_start,
        elements,
        uses_borrowed_backing: uses_borrowed_backing || !pointer_slots.is_empty(),
        pointer_slots,
        readable,
    })
}

/// Reconstruction scratch: offsets belong to separate storage regions around
/// pointer loads. The retained source path, not this scratch, is the evidence
/// from which legalization and instruction replay reconstruct the address.
pub(crate) struct PrimitiveAccessGeometry {
    pub(crate) byte_offset: u32,
    pub(crate) byte_size: u32,
    pub(crate) elements: Vec<RuntimeElement>,
    pub(crate) pointer_slots: Vec<PointerSlot>,
    /// The selected path crosses a descriptor, not merely an owner containing
    /// an unrelated borrowed field. This is geometry, never loan authority.
    pub(crate) uses_borrowed_backing: bool,
}

/// Atomic fields retain their declared identity while their address crosses
/// captured reference descriptors. This query reconstructs static geometry;
/// access ordering and current custody remain the atomic consumers' checks.
pub(crate) fn atomic_field_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<PrimitiveAccessGeometry> {
    let regions = storage_projection_regions(root, path, declarations)?;
    if !regions.readable || !regions.elements.is_empty() {
        return None;
    }
    let (byte_offset, byte_size, elements) = store(
        regions.root,
        &path[regions.segment_start..],
        field,
        scalar,
        declarations,
    )?;
    if !elements.is_empty() {
        return None;
    }
    Some(PrimitiveAccessGeometry {
        byte_offset,
        byte_size,
        elements,
        pointer_slots: regions.pointer_slots,
        uses_borrowed_backing: regions.uses_borrowed_backing,
    })
}

/// Reconstruct a primitive write without inventing a carrier record or field.
pub(crate) fn primitive_store(
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<PrimitiveAccessGeometry> {
    if destination.multiplicity() == terminal_psi::StructuralMultiplicity::Linear
        || (path.is_empty()
            && destination.multiplicity() != terminal_psi::StructuralMultiplicity::Unrestricted)
        || matches!(scalar, ScalarType::Integer(integer) if integer.is_address())
    {
        return None;
    }
    match destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { result, .. } => {
            // Whole primitive homes are the empty declared projection. The
            // common multiplicity fence above protects whole replacement;
            // geometry below authenticates the exact scalar endpoint.
            if !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
            {
                return None;
            }
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            parameter,
            ..
        } => {
            if parameter.access != terminal_psi::StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
        }
    }
    primitive_access_geometry(
        destination.structural_type(),
        path,
        scalar,
        declarations,
        primitive_geometry,
    )
}

/// Pointer crossings delimit storage regions for both reads and writes. The
/// endpoint resolver keeps each operation's field contract separate.
fn primitive_access_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
    endpoint: impl Fn(
        StructuralTypeId,
        &[StructuralPathSegment],
        ScalarType,
        &[StructuralTypeDeclaration],
    ) -> Option<(u32, u32, Vec<RuntimeElement>)>,
) -> Option<PrimitiveAccessGeometry> {
    let regions = storage_projection_regions(root, path, declarations)?;
    let (byte_offset, byte_size, leaf_elements) = endpoint(
        regions.root,
        &path[regions.segment_start..],
        scalar,
        declarations,
    )?;
    let mut elements = regions.elements;
    elements.extend(leaf_elements);
    Some(PrimitiveAccessGeometry {
        byte_offset,
        byte_size,
        elements,
        pointer_slots: regions.pointer_slots,
        uses_borrowed_backing: regions.uses_borrowed_backing,
    })
}

/// Reconstruct the selected primitive leaf and its footprint within the
/// original root: the static byte offset, the leaf width, and each runtime
/// element of the projection in path order. Every access the elements can
/// select must stay inside the root.
pub(crate) fn primitive_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32, Vec<RuntimeElement>)> {
    let mut roots = declarations
        .iter()
        .filter(|declaration| declaration.id == root);
    roots.next()?;
    if roots.next().is_some() {
        return None;
    }
    if let Some((selector, prefix)) = path.split_last()
        && matches!(
            selector,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
        )
        && let Some((selected, owner_offset, mut elements)) =
            runtime_projection(root, prefix, declarations)
        && let StructuralTypeShape::ByteSequence(carrier) =
            find_structural_type(declarations, selected)?.shape
    {
        if terminal_semantics::primitive_projection_type(declarations.iter(), root, path)
            != Some(scalar)
        {
            return None;
        }
        // A view's physical size describes its descriptor, never its payload
        // extent. Runtime bounds remain attached to the selected byte; an owner
        // additionally has an eight-byte length word before its payload.
        let (payload_offset, extent) = match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => (8u32, Some(capacity)),
            terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } => {
                (0u32, Some(capacity))
            }
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => (0u32, None),
        };
        let payload_offset = owner_offset.checked_add(payload_offset)?;
        return match selector {
            StructuralPathSegment::FixedIndex(index) => {
                if extent.is_some_and(|extent| *index >= extent) {
                    return None;
                }
                Some((
                    payload_offset.checked_add(u32::try_from(*index).ok()?)?,
                    1,
                    elements,
                ))
            }
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                elements.push(RuntimeElement {
                    index: *index,
                    obligation: *obligation,
                    stride: 1,
                    extent,
                });
                Some((payload_offset, 1, elements))
            }
            _ => None,
        };
    }
    if let Some((StructuralPathSegment::Field(identity), carrier_path)) = path.split_last() {
        let (carrier, _, _) = runtime_projection(root, carrier_path, declarations)?;
        if let Some(fields) = find_structural_type(declarations, carrier)?
            .shape
            .direct_fields()
        {
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
            if field.field_type.scalar_type() == Some(scalar) {
                return scalar_field_geometry(
                    root,
                    carrier_path,
                    field.id,
                    scalar,
                    declarations,
                    false,
                );
            }
        }
    }
    let (leaf, offset, elements) = runtime_projection(root, path, declarations)?;
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == leaf);
    let declaration = matches.next()?;
    if matches.next().is_some() || declaration.shape != StructuralTypeShape::PrimitiveScalar(scalar)
    {
        return None;
    }
    let bytes = scalar_shape(scalar)?.byte_size;
    scalar_access_within_storage(root, offset, bytes, &elements, declarations)
        .then_some((offset, bytes, elements))
}

/// Independently admit the complete primitive read before splitting its
/// pointer regions. Runtime selection before a reference crossing still
/// governs the read endpoint, even when the final region has no selector.
/// Region geometry retains separate pointer and scalar bounds; a store never
/// takes this route and keeps its own endpoint and proof requirements.
pub(crate) fn primitive_read_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<PrimitiveAccessGeometry> {
    if terminal_semantics::primitive_read_projection_type(declarations.iter(), root, path)
        != Some(scalar)
    {
        return None;
    }
    primitive_access_geometry(
        root,
        path,
        scalar,
        declarations,
        contiguous_primitive_read_geometry,
    )
}

fn contiguous_primitive_read_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32, Vec<RuntimeElement>)> {
    if let Some(geometry) = primitive_geometry(root, path, scalar, declarations) {
        return Some(geometry);
    }
    let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
        return None;
    };
    let field = terminal_semantics::runtime_structural_field(declarations.iter(), root, path)?;
    // The complete read established eligibility; this suffix independently
    // reconstructs the declared scalar width, field offset and region bounds.
    if field.field_type.scalar_type() != Some(scalar) {
        return None;
    }
    inline_scalar_field_geometry(root, carrier, field.id, scalar, declarations)
}

pub(crate) fn shape(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    shape_inner(root, declarations, &mut Vec::new())
}

/// Reconstruct parameter storage from its access, independently of body order.
pub(crate) fn parameter_shape<'declaration>(
    parameter: impl Into<abstract_operations::StructuralCallFormal<'declaration>>,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let parameter = parameter.into();
    // Whole-root domain qualifications are signature preconditions the caller
    // or root installation discharges at invocation; they do not change the
    // parameter's storage shape. Projected memberships are the same semantic
    // contract on a selected child, not an extra runtime permission carrier.
    if parameter.access == StructuralAccess::Owned
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
    {
        // Owned aggregate parameters use the same recursively retained payload
        // geometry as their local and result homes. Array element shape does
        // not change whole-value ownership or permit borrowed-pointer ABI.
        return owned_storage_shape(parameter.structural_type, declarations);
    }
    let referent = shape(parameter.structural_type, declarations)?;
    match (parameter.access, parameter.multiplicity) {
        // Restricted owners retain their exact claim and disposal obligations
        // in source custody; those obligations do not change payload geometry.
        (
            StructuralAccess::Owned,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Linear,
        ) => Some(referent),
        (
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow,
            terminal_psi::nonlinear_multiplicity_pair_pattern!(),
        ) => {
            // Carrier multiplicity controls lifetime, not the borrowed ABI.
            // In particular, an affine descriptor arrival has the same storage
            // geometry as an unrestricted arrival of the same type and access.
            internal_borrow_shape(parameter.structural_type, declarations)
        }
        _ => None,
    }
}

/// Internal loans select descriptor versus thin address by semantic referent,
/// never by the source's producer or a coincidental physical extent.
pub(crate) fn internal_borrow_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    if scalar_storage_type(structural_type, declarations).is_some() {
        Some(ValueShape::borrowed_scalar_reference())
    } else {
        let referent = shape(structural_type, declarations)?;
        Some(ValueShape::borrowed_reference(
            referent.byte_size,
            referent.alignment,
        ))
    }
}

/// Borrow geometry for an exact plain record, including nested owned records.
/// This does not authorize constructing those nested fields.
pub(crate) fn plain_record_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    find_structural_type(declarations, structural_type)
        .filter(|declaration| matches!(declaration.shape, StructuralTypeShape::Record { .. }))?;
    // Borrowing needs the complete referent footprint, including untouched byte
    // siblings. It does not need permission to construct or copy that payload.
    shape(structural_type, declarations)
}

/// Physical storage of a whole owned aggregate, independently of permission to
/// copy its contents. Existing erased fields occupy no bytes; nested loan
/// carriers retain their own geometry and independently checked custody. Owned
/// transport moves descriptor words, not their backing or loan authority; access
/// and multiplicity remain with the checked call, construction, or edge.
/// Scalar places and bare loan descriptors do not become aggregate owners.
pub(crate) fn owned_storage_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    find_structural_type(declarations, structural_type).filter(|declaration| {
        declaration.shape.is_composite_shape() || declaration.shape.is_bounded_owned_byte_sequence()
    })?;
    stored_value_shape(structural_type, declarations)
}

/// Complete stored-carrier geometry, including bare reference descriptors.
/// This describes storage only: owned aggregate/copy eligibility and exact
/// edge custody remain separate judgments at their respective consumers.
pub(crate) fn stored_value_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    primitive_array_shape(structural_type, declarations)
        .or_else(|| shape(structural_type, declarations))
}

/// Owned arrays retain their full recursive type, including below empty extents.
pub(crate) fn primitive_array_shape(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    let mut current = root;
    let mut count = Some(1_u64);
    let mut empty = false;
    let mut array = false;
    for _ in 0..declarations.len() {
        let mut matching = declarations
            .iter()
            .filter(|declaration| declaration.id == current);
        let declaration = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        match declaration.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                array = true;
                empty |= length == 0;
                count = count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(scalar) if array => {
                if matches!(scalar, ScalarType::Integer(integer) if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed)
                {
                    return None;
                }
                let leaf = scalar_shape(scalar)?;
                let size = u32::try_from(
                    (if empty { 0 } else { count? }).checked_mul(u64::from(leaf.byte_size))?,
                )
                .ok()?;
                return Some(ValueShape::integer(
                    size,
                    if size == 0 { 1 } else { leaf.alignment },
                ));
            }
            _ => return None,
        }
    }
    None
}

/// A byte-view boundary operand may present original fixed or live-length
/// owner storage. Reconstruct that presentation from source custody, not from
/// the target row's proposed descriptor extent.
pub(crate) fn borrowed_byte_view_presentation(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, Option<u64>, Option<u32>)> {
    if !declarations.iter().any(|declaration| {
        declaration.id == view_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
            )
    }) {
        return None;
    }
    if let Some((offset, length)) = fixed_byte_array_view(source, argument, view_type, declarations)
    {
        return Some((offset, Some(length), Some(1)));
    }
    // This foreign-ABI adapter currently transports a static window only.
    // Never erase runtime coordinates from an owner presentation.
    bounded_byte_view(source, argument, view_type, declarations)
        .filter(|geometry| geometry.elements.is_empty())
        .map(|geometry| (geometry.offset, None, None))
}

/// Reconstruct an initialized fixed-array loan, not an existing slice descriptor.
pub(crate) fn fixed_byte_array_view(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
    // A fixed semantic extent does not imply inline storage. An incoming
    // bounded view already owns its pointer presentation; forwarding it must
    // neither manufacture array-establishment metadata nor bound its backing
    // by the bytes occupied by that pointer cell.
    // The caller independently checks the actual callee signature. Here the
    // expected presentation is the exact plain borrowed byte-view shape.
    let mut destination = source.clone();
    destination.structural_type = view_type;
    destination.access = argument.access;
    destination.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
    let window = terminal_semantics::fixed_byte_array_window(
        declarations.iter(),
        source,
        argument,
        &destination,
    )?;
    let (backing, offset) = project(source.structural_type, window.backing_path, declarations)?;
    if terminal_semantics::borrowed_referent_presentation(declarations.iter(), backing, view_type)
        == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
    {
        return None;
    }
    let root = shape(source.structural_type, declarations)?;
    // Independently reconstruct the whole bounded view's pointer residence.
    // Its static referent extent must not be mistaken for inline carrier bytes.
    let stored_extent = if argument.path.is_empty()
        && matches!(
            terminal_psi::find_structural_type(declarations, source.structural_type)?.shape,
            terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
        ) {
        8
    } else {
        window.backing_length
    };
    if u64::from(offset).checked_add(stored_extent)? > u64::from(root.byte_size) {
        return None;
    }
    let offset = u64::from(offset).checked_add(window.offset)?;
    Some((u32::try_from(offset).ok()?, window.length))
}

/// Reconstruct a borrowed element view presented from a fixed array reached
/// through an authored field path. The element count and element byte size
/// are the descriptor's length word and stride; the window's end must stay
/// inside the source root storage.
pub(crate) fn fixed_element_array_view(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64, u32)> {
    let geometry = collection_view_geometry(source, argument, view_type, declarations)?;
    // A live-length carrier's descriptor extent is dynamic; this channel
    // only carries compile-known lengths.
    let terminal_semantics::CollectionExtent::Fixed(length) = geometry.extent else {
        return None;
    };
    (geometry.storage == terminal_semantics::CollectionStorage::Inline
        && geometry.elements.is_empty())
    .then_some((geometry.offset, length, geometry.stride))
}

/// Native storage coordinates for an ordinary collection establishment.
/// A borrowed collection stores a pointer at `offset`; its declared backing
/// extent is independent of the bytes occupied by that pointer cell. An
/// owned bounded byte carrier's live-length word precedes `offset`, so its
/// viewed bytes begin where the descriptor points.
pub(crate) struct CollectionViewGeometry {
    pub offset: u32,
    pub elements: Vec<RuntimeElement>,
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
                )))
        || !source.projected_qualifications.is_empty()
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
    let (collection_shape, offset, elements) =
        collection_projection(source.structural_type, field_path, declarations)?;
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
    let (selected_storage, _, _) =
        runtime_storage_projection(source.structural_type, field_path, declarations)?;
    let selected_size = field_shape(&selected_storage, declarations, &mut Vec::new())?.byte_size;
    let end = u64::from(window_offset).checked_add(storage_size)?;
    if end > u64::from(selected_size) {
        return None;
    }
    Some(CollectionViewGeometry {
        offset,
        elements,
        extent,
        stride: element_stride,
        storage: backing.storage,
    })
}

/// Reconstruct a borrowed view presented from whole or inline bounded-owner
/// storage. The live length word and capacity bytes stay in the caller's
/// storage; the descriptor staged for the call reads that length in place.
/// Shared presentation does not copy the owner's primitive predicate roster
/// onto the view formal; call semantics has already proved the view's weaker
/// promises. This returns metadata geometry only — it grants neither byte
/// access nor authority to replace the storage.
pub(crate) struct BoundedByteViewGeometry {
    pub(crate) offset: u32,
    pub(crate) capacity: u64,
    pub(crate) elements: Vec<RuntimeElement>,
}

pub(crate) fn bounded_byte_view(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<BoundedByteViewGeometry> {
    if source.place != argument.place
        || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
        || !source.projected_qualifications.is_empty()
    {
        return None;
    }
    // Exclusive borrowed-view fields retain their existing field-only route
    // and exact qualification restrictions. Only a shared presentation may
    // expose a whole owner without copying its primitive predicate roster onto
    // the view formal.
    if argument.access != terminal_psi::StructuralAccess::SharedBorrow {
        return bounded_byte_field_borrow(source, argument, view_type, declarations, false).map(
            |(offset, capacity)| BoundedByteViewGeometry {
                offset,
                capacity,
                elements: Vec::new(),
            },
        );
    }
    let expected = terminal_psi::StructuralParameterDeclaration {
        place: argument.place,
        position: 0,
        structural_type: view_type,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        access: terminal_psi::StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let capacity = terminal_semantics::shared_boundary_buffer_capacity(
        declarations.iter(),
        source.structural_type,
        argument,
        &expected,
    )?;
    // A stored view's selector crosses backing storage, even without Referent.
    // A descriptor-relative offset cannot describe that owner's live header.
    if !storage_projection_regions(source.structural_type, &argument.path, declarations)?
        .pointer_slots
        .is_empty()
    {
        return None;
    }
    let (selected, offset, elements) =
        collection_projection(source.structural_type, &argument.path, declarations)?;
    if *selected
        != StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity,
        })
        || !runtime_span_within_storage(
            u64::from(offset),
            8u64.checked_add(capacity)?,
            &elements,
            u64::from(shape(source.structural_type, declarations)?.byte_size),
        )
    {
        return None;
    }
    Some(BoundedByteViewGeometry {
        offset,
        capacity,
        elements,
    })
}

pub(crate) fn bounded_byte_field_owner(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
    bounded_byte_field_borrow(source, argument, view_type, declarations, true)
}

fn bounded_byte_field_borrow(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    owned: bool,
) -> Option<(u32, u64)> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let Some((StructuralPathSegment::Field(_), _)) = argument.path.split_last() else {
        return None;
    };
    let shared_owned_storage = !owned
        && source.access == StructuralAccess::Owned
        && argument.access == StructuralAccess::SharedBorrow;
    if source.place != argument.place
        || !(shared_owned_storage
            || matches!(
                (source.access, argument.access),
                (
                    StructuralAccess::MutableBorrow,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow,
                ) | (
                    StructuralAccess::SharedBorrow,
                    StructuralAccess::SharedBorrow
                ) | (
                    StructuralAccess::WriteOnlyBorrow,
                    StructuralAccess::WriteOnlyBorrow
                )
            ))
        || source.multiplicity != StructuralMultiplicity::Unrestricted
        || (owned || argument.access != StructuralAccess::SharedBorrow)
            && !source.qualifications.is_empty()
        || !source.projected_qualifications.is_empty()
        || !declarations.iter().any(|declaration| {
            declaration.id == view_type
                && if owned {
                    declaration.shape.is_bounded_owned_byte_sequence()
                } else {
                    declaration.shape.is_borrowed_byte_carrier()
                }
        })
    {
        return None;
    }
    if !owned && argument.access == StructuralAccess::WriteOnlyBorrow {
        return None;
    }
    let (selected, field_offset, elements) =
        collection_projection(source.structural_type, &argument.path, declarations)?;
    let StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    }) = *selected
    else {
        return None;
    };
    if !elements.is_empty()
        || !storage_projection_regions(source.structural_type, &argument.path, declarations)?
            .pointer_slots
            .is_empty()
        || (owned && find_structural_type(declarations, view_type)?.shape != *selected)
    {
        return None;
    }
    (u64::from(field_offset)
        .checked_add(8)?
        .checked_add(capacity)?
        <= u64::from(shape(source.structural_type, declarations)?.byte_size))
    .then_some((field_offset, capacity))
}

/// Per-case payload shapes in declaration order with relevance-erased fields
/// dropped — the same projection the sum and mixed layouts evaluate.
fn case_payloads(
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> Option<Vec<Vec<ValueShape>>> {
    cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| field_shape(&field.field_type, declarations, active))
                .collect::<Option<Vec<_>>>()
        })
        .collect()
}

fn shape_inner(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    if active.contains(&root) {
        return None;
    }
    active.push(root);
    let declaration = find_structural_type(declarations, root)?;
    let result = match &declaration.shape {
        // Borrowing preserves a pointer to the established occurrence even
        // though the service marker itself contributes no payload bytes.
        StructuralTypeShape::BindingCarrier => Some(ValueShape::integer(0, 1)),
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_shape(*scalar),
        StructuralTypeShape::ByteSequence(carrier) => field_shape(
            &StructuralFieldType::ByteSequence(*carrier),
            declarations,
            active,
        ),
        // An element view rides the same borrowed descriptor pair: base
        // pointer plus extent, realized as one borrowed aggregate slot.
        StructuralTypeShape::ElementView { .. } => Some(ValueShape::integer(16, 8)),
        StructuralTypeShape::Sum { cases } => {
            let payloads = case_payloads(cases, declarations, active)?;
            Some(
                calling_conventions::evaluate_conventional_sum_layout(&[], &payloads)
                    .ok()?
                    .shape,
            )
        }
        // A mixed declaration keeps its tag prefix, common fields, and
        // overlaid case payloads. Storage geometry uses the same leaf
        // coverage as record fields; observing its tag or projecting a common
        // field remains a separate, narrower admission.
        StructuralTypeShape::Mixed { fields, cases } => {
            let common = fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| field_shape(&field.field_type, declarations, active))
                .collect::<Option<Vec<_>>>()?;
            let payloads = case_payloads(cases, declarations, active)?;
            Some(
                calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
                    .ok()?
                    .shape,
            )
        }
        // A reference owns permission, not its referent's bytes. Its payload
        // transports one address; custody remains independently reconstructed.
        StructuralTypeShape::Reference { referent, .. } => Some(ValueShape::integer(
            if scalar_storage_type(*referent, declarations).is_some() {
                16
            } else {
                8
            },
            8,
        )),
        StructuralTypeShape::Record { fields, storage } => {
            if matches!(
                storage,
                terminal_psi::StructuralRecordStorage::TargetLayout(_)
            ) {
                return None;
            }
            if let terminal_psi::StructuralRecordStorage::Authored(storage)
            | terminal_psi::StructuralRecordStorage::PlacedView(storage)
            | terminal_psi::StructuralRecordStorage::PlacedAccessor(storage) = storage
            {
                active.pop();
                return Some(ValueShape::integer(
                    u32::try_from(storage.byte_size).ok()?,
                    u16::try_from(storage.byte_alignment).ok()?,
                ));
            }
            let mut bytes = 0;
            let mut alignment = 1;
            for field in fields.iter().filter(|field| {
                !field.relevance.is_erased()
                    && !matches!(field.field_type, StructuralFieldType::Erased { .. })
            }) {
                let field_shape = field_shape(&field.field_type, declarations, active)?;
                alignment = alignment.max(field_shape.alignment);
                bytes = align(bytes, field_shape.alignment)?.checked_add(field_shape.byte_size)?;
            }
            Some(ValueShape::integer(align(bytes, alignment)?, alignment))
        }
        StructuralTypeShape::FixedArray { element, length } => {
            // Owned primitive arrays already define their recursive ABI,
            // including zero-byte arrays. Borrowing must observe that same
            // layout instead of independently imposing a nonempty extent.
            if let Some(shape) = primitive_array_shape(root, declarations) {
                Some(shape)
            } else {
                let element = shape_inner(*element, declarations, active)?;
                let stride = align(element.byte_size, element.alignment)?;
                Some(ValueShape::integer(
                    u32::try_from(u64::from(stride).checked_mul(*length)?).ok()?,
                    if *length == 0 { 1 } else { element.alignment },
                ))
            }
        }
    };
    active.pop();
    result
}

pub(crate) fn field_storage_shape(
    field: &terminal_psi::StructuralFieldDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    if matches!(
        field.storage.as_ref(),
        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
    ) {
        return None;
    }
    if let Some(stored) = field
        .storage
        .as_ref()
        .and_then(|storage| storage.stored_scalar())
    {
        let codec = runtime_abi::ScalarStorageCodec::from_scalar(stored)?;
        return Some(ValueShape::integer(u32::from(codec.byte_size()), 1));
    }
    field_shape(&field.field_type, declarations, &mut Vec::new())
}

fn field_storage_offset(
    field: &terminal_psi::StructuralFieldDeclaration,
    conventional_offset: u32,
    alignment: u16,
) -> Option<u32> {
    match field.storage.as_ref() {
        Some(storage) => u32::try_from(storage.byte_offset()?).ok(),
        None => align(conventional_offset, alignment),
    }
}

/// Interpretation at a direct scalar endpoint, read from its admitted plan.
pub(crate) fn projected_scalar_codec(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<runtime_abi::ScalarStorageCodec> {
    let (root, path) = referent_projection(root, path, declarations)?;
    if let Some((StructuralPathSegment::Field(_), prefix)) = path.split_last() {
        let field = terminal_semantics::runtime_structural_field(declarations.iter(), root, path)?;
        return scalar_field_codec(root, prefix, field.id, declarations);
    }
    // A primitive leaf is described by its scalar storage alone. A byte
    // element in particular has no endpoint declaration, and the catalog need
    // not contain one for the program to read it.
    let (storage, _, _) = runtime_storage_projection(root, path, declarations)?;
    let scalar = match &*storage {
        StructuralFieldType::Scalar(scalar) => *scalar,
        StructuralFieldType::Structural(selected) => scalar_storage_type(*selected, declarations)?,
        field => scalar_storage_type(
            field
                .canonical_leaf_type()?
                .leaf_declaration(declarations.iter())?
                .id,
            declarations,
        )?,
    };
    runtime_abi::ScalarStorageCodec::from_scalar(scalar)
}

/// Primitive path observations retain the same recipe as named scalar-field
/// observations. Reference crossings remain ordered; no container envelope
/// is substituted for a contiguous leaf address.
pub(crate) fn projected_scalar_fragments<'declarations>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &'declarations [StructuralTypeDeclaration],
) -> Option<&'declarations semantic_vocabulary::AuthoredBitFragments> {
    let (root, path) = referent_projection(root, path, declarations)?;
    let (StructuralPathSegment::Field(_), prefix) = path.split_last()? else {
        return None;
    };
    let field = terminal_semantics::runtime_structural_field(declarations.iter(), root, path)?;
    scalar_field_fragments(root, prefix, field.id, declarations)
}

fn referent_projection<'path>(
    root: StructuralTypeId,
    path: &'path [StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    // Codecs and fragmented-field recipes belong to the final storage region,
    // after the same pointer loads used to compute the actual address.
    let regions = storage_projection_regions(root, path, declarations)?;
    Some((regions.root, &path[regions.segment_start..]))
}

pub(crate) fn scalar_field_codec(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<runtime_abi::ScalarStorageCodec> {
    let field = scalar_field_declaration(root, path, field, declarations)?;
    if matches!(
        field.storage.as_ref(),
        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
    ) {
        return None;
    }
    runtime_abi::ScalarStorageCodec::from_scalar(
        field
            .storage
            .as_ref()
            .and_then(|storage| storage.stored_scalar())
            .or_else(|| field.field_type.scalar_type())?,
    )
}

/// Retain a fragmented field's exact authored recipe after ordered reference
/// crossings. Its container envelope never becomes a scalar-reference codec.
pub(crate) fn scalar_field_fragments<'declarations>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &'declarations [StructuralTypeDeclaration],
) -> Option<&'declarations semantic_vocabulary::AuthoredBitFragments> {
    let field = scalar_field_declaration(root, path, field, declarations)?;
    if matches!(field.field_type, StructuralFieldType::Atomic(_)) {
        return None;
    }
    let semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments) =
        field.storage.as_ref()?
    else {
        return None;
    };
    Some(fragments)
}

fn scalar_field_declaration<'declarations>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &'declarations [StructuralTypeDeclaration],
) -> Option<&'declarations terminal_psi::StructuralFieldDeclaration> {
    let (root, path) = referent_projection(root, path, declarations)?;
    terminal_semantics::projected_field_declaration(declarations.iter(), root, path, field)
}

pub(crate) fn field_shape(
    field: &StructuralFieldType,
    declarations: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    match field {
        StructuralFieldType::Scalar(scalar) => scalar_shape(*scalar),
        StructuralFieldType::Atomic(_) => scalar_shape(field.atomic_scalar_type()?),
        // Geometry follows the carrier, not the restriction's endpoints.
        // Store admission below still requires separate restricted-field evidence.
        StructuralFieldType::BoundedInteger(bounds) => {
            scalar_shape(ScalarType::Integer(bounds.integer_type()))
        }
        StructuralFieldType::RefinedScalar(refinement) => scalar_shape(refinement.carrier()),
        StructuralFieldType::IeeeFloat(format) => scalar_shape(ScalarType::IeeeFloat(*format)),
        StructuralFieldType::ByteSequence(carrier)
        | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
            // Match target layout: an inline bounded buffer retains capacity
            // bytes and a length word; a borrowed view retains two words. This
            // supplies sibling offsets only, not byte access or replacement.
            let bytes = match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    capacity.checked_add(8)?
                }
                // A bounded view's extent lives in the declaration; the
                // stored descriptor is the bare referent pointer.
                terminal_psi::ByteSequenceCarrier::BoundedView { .. } => 8,
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 16,
            };
            Some(ValueShape::integer(u32::try_from(bytes).ok()?, 8))
        }
        StructuralFieldType::Structural(nested) => shape_inner(*nested, declarations, active),
        _ => None,
    }
}

/// Independently reconstruct case-qualified copy geometry from declarations;
/// neither a producer's offset nor its selected-case assertion is layout evidence.
pub(crate) fn case_projection(
    root: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32)> {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    let endpoint = terminal_semantics::canonical_case_leaf_type(declarations.iter(), root, path)?;
    let mut current = root;
    let mut offset = 0u32;
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        let declaration = find_structural_type(declarations, current)?;
        let (next, displacement) = match segment {
            Segment::Case(identity) => {
                let cases = declaration.shape.sum_cases()?;
                let case_position = structural_case_position(cases, *identity)?;
                let Segment::Field(field) = segments.next()? else {
                    return None;
                };
                let (field_position, field) = cases[case_position]
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, candidate)| candidate.id == *field)?;
                let next = field.field_type.as_structural()?;
                let layout = case_layout(current, declarations)?;
                (
                    next,
                    layout
                        .cases
                        .get(case_position)?
                        .fields
                        .get(field_position)?
                        .byte_offset,
                )
            }
            Segment::Field(identity) => {
                let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &declaration.shape
                else {
                    return None;
                };
                let (field_position, field) = fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.id == *identity)?;
                let next = field.field_type.as_structural()?;
                let displacement = if matches!(declaration.shape, StructuralTypeShape::Mixed { .. })
                {
                    case_layout(current, declarations)?
                        .common_fields
                        .get(field_position)?
                        .byte_offset
                } else {
                    project(
                        current,
                        &[StructuralPathSegment::Field(field.identity.clone())],
                        declarations,
                    )?
                    .1
                };
                (next, displacement)
            }
            Segment::FixedIndex(index) => project(
                current,
                &[StructuralPathSegment::FixedIndex(*index)],
                declarations,
            )?,
        };
        current = next;
        offset = offset.checked_add(displacement)?;
    }
    (current == endpoint).then_some((endpoint, offset))
}

/// Reconstruct tag, common-field and case-payload geometry without granting
/// authority to select or access any case.
pub(crate) fn case_layout(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<calling_conventions::ConventionalSumLayout> {
    let declaration = find_structural_type(declarations, root)?;
    let (common, cases) = match &declaration.shape {
        StructuralTypeShape::Sum { cases } => (&[][..], cases),
        StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
        _ => return None,
    };
    let mut active = vec![root];
    let common = common
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| field_shape(&field.field_type, declarations, &mut active))
        .collect::<Option<Vec<_>>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| field_shape(&field.field_type, declarations, &mut active))
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&common, &payloads).ok()
}

/// Geometry of a borrowed path. Each descriptor offset belongs to the
/// storage reached by the preceding pointer load, not the original home.
pub(crate) struct ReferenceProjection {
    pub(crate) selected: StructuralTypeId,
    pub(crate) pointer_slots: Vec<PointerSlot>,
    pub(crate) elements: Vec<RuntimeElement>,
    pub(crate) referent_offset: u32,
    /// The final crossing reaches a scalar descriptor, not aggregate backing.
    pub(crate) scalar_descriptor: bool,
}

/// Resolve stored reference and view crossings without choosing a backing origin.
/// This checks declared access and layout only; callers independently retain
/// live loan custody and the identity of the original carrier home.
pub(crate) fn reference_projection(
    mut root: StructuralTypeId,
    mut path: &[StructuralPathSegment],
    access: terminal_psi::StructuralAccess,
    expected: Option<StructuralTypeId>,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ReferenceProjection> {
    use terminal_psi::StructuralAccess;
    // Each region is resolved by the ordinary runtime storage projection below.
    // A second path grammar would discard case namespaces before that owner
    // can join the selected payload to its actual descriptor offset.
    if access == StructuralAccess::Owned {
        return None;
    }
    let initial_root = root;
    let initial_path = path;
    while let Some(crossing) = path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)
    {
        let (carrier, _, _) = runtime_projection(root, &path[..crossing], declarations)?;
        let StructuralTypeShape::Reference {
            referent,
            access: granted,
        } = find_structural_type(declarations, carrier)?.shape
        else {
            return None;
        };
        if !(granted == access
            || granted == StructuralAccess::MutableBorrow
                && matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
        {
            return None;
        }
        if granted == StructuralAccess::WriteOnlyBorrow
            && path[crossing + 1..].contains(&StructuralPathSegment::Referent)
        {
            return None;
        }
        root = referent;
        path = &path[crossing + 1..];
    }
    let expected = expected.and_then(|identity| find_structural_type(declarations, identity));
    let geometry = reference_geometry(initial_root, initial_path, declarations, |root, path| {
        runtime_projection_with_leaf_type(
            root,
            path,
            declarations,
            borrowed_leaf_type(access, expected),
        )
    })?;
    let scalar_descriptor = geometry.pointer_slots.last()?.scalar_descriptor;
    Some(ReferenceProjection {
        selected: geometry.selected,
        pointer_slots: geometry.pointer_slots,
        elements: geometry.elements,
        referent_offset: geometry.byte_offset,
        scalar_descriptor,
    })
}

pub(crate) fn project(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32)> {
    project_with_leaf_type(
        root,
        path,
        declarations,
        StructuralFieldType::canonical_leaf_type,
    )
}

fn borrowed_leaf_type(
    access: terminal_psi::StructuralAccess,
    expected: Option<&StructuralTypeDeclaration>,
) -> impl Fn(&StructuralFieldType) -> Option<StructuralFieldType> + Copy + '_ {
    move |field| field.borrowed_presentation_type(access, expected)
}

/// Byte-carrier leaves resolve to the standalone declaration a leaf copy of
/// them mints — the same storage projection, extended past plain leaves.
pub(crate) fn leaf_copy_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32)> {
    project_with_leaf_type(
        root,
        path,
        declarations,
        StructuralFieldType::leaf_copy_type,
    )
}

fn project_with_leaf_type(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
    leaf_type: fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Option<(StructuralTypeId, u32)> {
    project_inner(
        root,
        path.iter().map(Projection::from),
        declarations,
        leaf_type,
    )
    .and_then(|(selected, offset, elements)| elements.is_empty().then_some((selected, offset)))
}

enum Projection {
    Case(semantic_vocabulary::StructuralCaseId),
    FieldName(terminal_psi::TerminalIdentityIndex),
    FixedIndex(u64),
    RuntimeIndex(
        semantic_vocabulary::ValueId,
        semantic_vocabulary::ObligationId,
    ),
    Unsupported,
}

impl From<&StructuralPathSegment> for Projection {
    fn from(segment: &StructuralPathSegment) -> Self {
        match segment {
            StructuralPathSegment::Field(name) => Self::FieldName(*name),
            StructuralPathSegment::Case(case) => Self::Case(*case),
            StructuralPathSegment::FixedIndex(position) => Self::FixedIndex(*position),
            StructuralPathSegment::RuntimeIndex { index, obligation } => {
                Self::RuntimeIndex(*index, *obligation)
            }
            _ => Self::Unsupported,
        }
    }
}

/// Select a collection occurrence for production or independent layout audit.
/// Inline byte fields contribute their carrier directly; a catalog declaration
/// with equal geometry is neither necessary nor an identity for that field.
pub(crate) fn collection_projection<'types>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &'types [StructuralTypeDeclaration],
) -> Option<(
    std::borrow::Cow<'types, StructuralTypeShape>,
    u32,
    Vec<RuntimeElement>,
)> {
    let (storage, offset, elements) =
        project_storage(root, path.iter().map(Projection::from), declarations)?;
    let shape = match &*storage {
        StructuralFieldType::Structural(identity) => {
            std::borrow::Cow::Borrowed(&find_structural_type(declarations, *identity)?.shape)
        }
        field => std::borrow::Cow::Owned(StructuralTypeShape::ByteSequence(
            field.byte_sequence_carrier()?,
        )),
    };
    Some((shape, offset, elements))
}

/// Selector geometry depends on the actual storage occurrence, not on whether
/// its final inline leaf has a standalone declaration. Access admission belongs
/// to the consuming operation; this query grants no leaf presentation.
pub(crate) fn runtime_projection_elements(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<Vec<RuntimeElement>> {
    let regions = storage_projection_regions(root, path, declarations)?;
    let (_, _, leaf_elements) =
        runtime_storage_projection(regions.root, &path[regions.segment_start..], declarations)?;
    let mut elements = regions.elements;
    elements.extend(leaf_elements);
    Some(elements)
}

fn project_inner(
    root: StructuralTypeId,
    path: impl Iterator<Item = Projection>,
    declarations: &[StructuralTypeDeclaration],
    leaf_type: fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
    let (storage, offset, elements) = project_storage(root, path, declarations)?;
    let identity = match &*storage {
        StructuralFieldType::Structural(identity) => *identity,
        field => match leaf_type(field)? {
            StructuralFieldType::Structural(identity) => identity,
            field => field.leaf_declaration(declarations.iter())?.id,
        },
    };
    Some((identity, offset, elements))
}

/// Walk physical storage without inventing a declaration for an inline leaf.
/// A qualified owner and an unqualified temporary may share capacity; neither
/// identity substitutes for the selected field when forming its view.
fn project_storage<'types>(
    root: StructuralTypeId,
    path: impl Iterator<Item = Projection>,
    declarations: &'types [StructuralTypeDeclaration],
) -> Option<(
    std::borrow::Cow<'types, StructuralFieldType>,
    u32,
    Vec<RuntimeElement>,
)> {
    use std::borrow::Cow;
    let mut storage: Cow<'_, StructuralFieldType> =
        Cow::Owned(StructuralFieldType::Structural(root));
    let mut offset = 0_u32;
    let mut elements = Vec::new();
    let mut repeated_stride = None;
    shape(root, declarations)?;
    let mut path = path;
    while let Some(segment) = path.next() {
        let carrier = storage.as_structural()?;
        let declaration = find_structural_type(declarations, carrier)?;
        match (segment, &declaration.shape) {
            (
                Projection::Case(case),
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
            ) => {
                let case_position = structural_case_position(cases, case)?;
                let Projection::FieldName(identity) = path.next()? else {
                    return None;
                };
                let (field_position, field) =
                    terminal_psi::live_field_entry(&cases[case_position].fields, identity)?;
                let layout = case_layout(carrier, declarations)?;
                offset = offset.checked_add(
                    layout
                        .cases
                        .get(case_position)?
                        .fields
                        .get(field_position)?
                        .byte_offset,
                )?;
                storage = Cow::Borrowed(&field.field_type);
                repeated_stride = field
                    .storage
                    .as_ref()
                    .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride);
            }
            (selection @ (Projection::FixedIndex(_) | Projection::RuntimeIndex(..)), array) => {
                let (element, extent) = match array {
                    StructuralTypeShape::FixedArray { element, length } => {
                        (*element, Some(*length))
                    }
                    StructuralTypeShape::ElementView { element, .. } => (*element, None),
                    _ => return None,
                };
                let element_shape = shape(element, declarations)?;
                let stride = match repeated_stride.take() {
                    Some(stride) => u32::try_from(stride).ok()?,
                    None => align(element_shape.byte_size, element_shape.alignment)?,
                };
                match selection {
                    Projection::FixedIndex(position) => {
                        if extent.is_some_and(|length| position >= length) {
                            return None;
                        }
                        offset = offset.checked_add(
                            u32::try_from(u64::from(stride).checked_mul(position)?).ok()?,
                        )?;
                    }
                    Projection::RuntimeIndex(index, obligation) => elements.push(RuntimeElement {
                        index,
                        obligation,
                        stride,
                        extent,
                    }),
                    _ => return None,
                }
                storage = Cow::Owned(StructuralFieldType::Structural(element));
            }
            (Projection::FieldName(name), StructuralTypeShape::Mixed { fields, .. }) => {
                let (position, field) = terminal_psi::live_field_entry(fields, name)?;
                let field_offset = case_layout(carrier, declarations)?
                    .common_fields
                    .get(position)?
                    .byte_offset;
                storage = Cow::Borrowed(&field.field_type);
                offset = offset.checked_add(field_offset)?;
                repeated_stride = field
                    .storage
                    .as_ref()
                    .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride);
            }
            (Projection::FieldName(name), StructuralTypeShape::Record { fields, .. }) => {
                let mut field_offset = 0;
                let mut found = None;
                for field in fields.iter().filter(|field| {
                    !field.relevance.is_erased()
                        && !matches!(field.field_type, StructuralFieldType::Erased { .. })
                }) {
                    if field.storage.is_some() && field.identity != name {
                        continue;
                    }
                    let layout = field_storage_shape(field, declarations)?;
                    field_offset = field_storage_offset(field, field_offset, layout.alignment)?;
                    if field.identity == name {
                        found = Some((
                            &field.field_type,
                            field_offset,
                            field.storage.as_ref().and_then(
                                semantic_vocabulary::AuthoredFieldStorage::element_stride,
                            ),
                        ));
                        break;
                    }
                    field_offset = field_offset.checked_add(layout.byte_size)?;
                }
                let (nested, field_offset, stride) = found?;
                storage = Cow::Borrowed(nested);
                offset = offset.checked_add(field_offset)?;
                repeated_stride = stride;
            }
            _ => return None,
        }
    }
    Some((storage, offset, elements))
}

/// Resolve a projection that may traverse `RuntimeIndex` segments at any
/// depth — a leaf copy's extent, a primitive leaf, or a field store's
/// carrier. Static segments before, between, and after the runtime elements
/// fold into the returned byte offset; each runtime element contributes its
/// selector, obligation, stride and extent so the access scales every
/// operand into the same address in path order. The element's bound is the
/// obligation the terminal verifier discharged; only array shape is
/// resolved here.
pub(crate) fn runtime_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
    runtime_projection_with_leaf_type(
        root,
        path,
        declarations,
        StructuralFieldType::canonical_leaf_type,
    )
}

/// Reconstruct a loan's exact storage geometry using its admitted access.
/// Primitive observation changes only the endpoint declaration, not the root,
/// field placement, selectors, qualifications, or authority held by the caller.
pub(crate) fn runtime_borrowed_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: terminal_psi::StructuralAccess,
    expected: Option<StructuralTypeId>,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
    let expected = expected.and_then(|expected| find_structural_type(declarations, expected));
    let geometry = reference_geometry(root, path, declarations, |root, path| {
        runtime_projection_with_leaf_type(
            root,
            path,
            declarations,
            borrowed_leaf_type(access, expected),
        )
    })?;
    Some((geometry.selected, geometry.byte_offset, geometry.elements))
}

fn runtime_projection_with_leaf_type(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
    leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
    let (storage, offset, elements) = runtime_storage_projection(root, path, declarations)?;
    let selected = match &*storage {
        StructuralFieldType::Structural(identity) => *identity,
        field => match leaf_type(field)? {
            StructuralFieldType::Structural(identity) => identity,
            field => field.leaf_declaration(declarations.iter())?.id,
        },
    };
    Some((selected, offset, elements))
}

/// A byte element uses its primitive storage geometry; aggregate projections
/// retain the selected field contract. Neither needs an endpoint declaration
/// until a consumer requests a typed presentation.
fn runtime_storage_projection<'types>(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &'types [StructuralTypeDeclaration],
) -> Option<(
    std::borrow::Cow<'types, StructuralFieldType>,
    u32,
    Vec<RuntimeElement>,
)> {
    // Byte elements have primitive storage without a standalone declaration.
    // Classify the selected collection, not the outer root: a record field or
    // array element retains the same byte storage and codec as a direct owner.
    if let Some((selector, prefix)) = path.split_last()
        && matches!(
            selector,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
        )
        && collection_projection(root, prefix, declarations)
            .is_some_and(|(shape, _, _)| matches!(*shape, StructuralTypeShape::ByteSequence(_)))
    {
        let scalar =
            terminal_semantics::primitive_projection_type(declarations.iter(), root, path)?;
        let (offset, _, elements) = primitive_geometry(root, path, scalar, declarations)?;
        return Some((
            std::borrow::Cow::Owned(StructuralFieldType::Scalar(scalar)),
            offset,
            elements,
        ));
    }
    // Static and runtime selectors consume the same retained field placement.
    // Splitting a path into static runs discarded authored repeated strides.
    project_storage(root, path.iter().map(Projection::from), declarations)
}

/// The field-store geometry: the static offset of `field` beneath the
/// carrier `path`, its width, and each runtime element of the carrier in
/// path order. The carrier composes record fields and literal or runtime
/// elements in any order (`is_structural_scalar_store_path`).
/// Reconstruct every pointer region before selecting the final scalar field.
/// The pointer offsets and selector regions survive to instruction selection;
/// flattening them would write the carrier home instead of its referent.
pub(crate) fn field_store_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<PrimitiveAccessGeometry> {
    let regions = storage_projection_regions(root, path, declarations)?;
    let (byte_offset, byte_size, leaf_elements) = inline_scalar_field_geometry(
        regions.root,
        &path[regions.segment_start..],
        field,
        scalar,
        declarations,
    )?;
    let mut elements = regions.elements;
    elements.extend(leaf_elements);
    Some(PrimitiveAccessGeometry {
        byte_offset,
        byte_size,
        elements,
        pointer_slots: regions.pointer_slots,
        uses_borrowed_backing: regions.uses_borrowed_backing,
    })
}

pub(crate) fn store(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32, Vec<RuntimeElement>)> {
    let geometry = field_store_geometry(root, path, field, scalar, declarations)?;
    Some((geometry.byte_offset, geometry.byte_size, geometry.elements))
}

fn inline_scalar_field_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32, Vec<RuntimeElement>)> {
    // A final Case supplies the payload namespace of the separate leaf field.
    // This is address reconstruction only; callers establish live authority.
    if let Some((StructuralPathSegment::Case(case), prefix)) = path.split_last() {
        let (carrier, carrier_offset, elements) = runtime_projection(root, prefix, declarations)?;
        let (offset, bytes) = case_field_read(carrier, &[], *case, field, scalar, declarations)?;
        return Some((carrier_offset.checked_add(offset)?, bytes, elements));
    }
    // The carrier resolver checks each declaration-owned segment, including
    // Case/Field pairs below a selected payload. A second record-only path
    // grammar would reject that same field solely because of its container.
    // A mixed shape's common field is written in place, beside the tag, with
    // no case knowledge: the same geometry its read observes.
    if let Some((carrier, carrier_offset, elements)) = runtime_projection(root, path, declarations)
        && let Some((offset, bytes)) =
            mixed_common_field_geometry(carrier, field, scalar, declarations)
    {
        return Some((carrier_offset.checked_add(offset)?, bytes, elements));
    }
    // Current-IR validation binds every refined write to its accepted predicate
    // proposition. This helper checks physical geometry, not proof authority.
    scalar_field_geometry(root, path, field, scalar, declarations, false)
}

/// Independently reconstruct the scalar leaf's offset and width inside one
/// element. The descriptor's element stride is not the width of this access.
pub(crate) fn element_leaf_geometry(
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32)> {
    let (offset, bytes, elements) = element_leaf_projection(element, path, scalar, declarations)?;
    // Layout disclosures and typed reinterpretations describe a static leaf.
    // Dynamic reads must retain the selectors, not discard their displacement.
    elements.is_empty().then_some((offset, bytes))
}

/// A view's outer selector uses its live length. Below that selected element,
/// reuse ordinary record/array geometry and retain every descendant selector;
/// returning only a static offset would silently read element zero.
pub(crate) fn element_leaf_projection(
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32, Vec<RuntimeElement>)> {
    if terminal_semantics::element_view_leaf_scalar(declarations, element, path) != Some(scalar) {
        return None;
    }
    if let Some((StructuralPathSegment::Field(identity), prefix)) = path.split_last() {
        let (parent, _, _) = runtime_projection(element, prefix, declarations)?;
        let StructuralTypeShape::Record { fields, .. } =
            &find_structural_type(declarations, parent)?.shape
        else {
            return None;
        };
        let field = terminal_psi::field_by_identity(fields, *identity)?;
        if field.field_type.scalar_type() == Some(scalar) {
            return scalar_field_geometry(element, prefix, field.id, scalar, declarations, false);
        }
    }
    primitive_geometry(element, path, scalar, declarations)
}

/// Reads and proven writes share carrier geometry. Neither this layout nor a
/// constructor/entry predicate proof supplies the separate authority for a write.
fn scalar_field_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
    observes_byte_length: bool,
) -> Option<(u32, u32, Vec<RuntimeElement>)> {
    let (carrier, carrier_offset, elements) = runtime_projection(root, path, declarations)?;
    if matches!(
        find_structural_type(declarations, carrier)?.shape,
        StructuralTypeShape::Mixed { .. }
    ) {
        // A selected array element retains its own common-field placement
        // after the tag. Reconstruct that offset independently, keeping the
        // carrier's runtime selectors and enclosing storage bounds.
        if observes_byte_length {
            return None;
        }
        let (offset, bytes) = mixed_common_field_geometry(carrier, field, scalar, declarations)?;
        let offset = carrier_offset.checked_add(offset)?;
        return scalar_access_within_storage(root, offset, bytes, &elements, declarations)
            .then_some((offset, bytes, elements));
    }
    let StructuralTypeShape::Record { fields, .. } =
        &find_structural_type(declarations, carrier)?.shape
    else {
        return None;
    };
    let mut offset = 0;
    for candidate in fields.iter().filter(|field| {
        !field.relevance.is_erased()
            && !matches!(field.field_type, StructuralFieldType::Erased { .. })
    }) {
        if candidate.storage.is_some() && candidate.id != field {
            continue;
        }
        if candidate.id == field
            && let Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) =
                candidate.storage.as_ref()
        {
            if observes_byte_length
                || matches!(candidate.field_type, StructuralFieldType::Atomic(_))
                || candidate.field_type.scalar_type() != Some(scalar)
            {
                return None;
            }
            let (start, bytes) = fragments.byte_envelope()?;
            let offset = carrier_offset.checked_add(u32::try_from(start).ok()?)?;
            let bytes = u32::try_from(bytes).ok()?;
            return scalar_access_within_storage(root, offset, bytes, &elements, declarations)
                .then_some((offset, bytes, elements));
        }
        let layout = field_storage_shape(candidate, declarations)?;
        offset = field_storage_offset(candidate, offset, layout.alignment)?;
        if candidate.id == field {
            // A bounded owner's live length is its leading metadata word; a
            // shared view field's is the extent word after its pointer.
            let mut length_word = 0;
            let matches_type = match candidate.field_type {
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ..
                } => {
                    observes_byte_length
                        && matches!(scalar, ScalarType::Integer(integer)
                        if Ok(integer) == semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned, 64))
                }
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    },
                )
                | StructuralFieldType::PredicateByteSequence {
                    carrier:
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                        },
                    ..
                } => {
                    length_word = 8;
                    observes_byte_length
                        && matches!(scalar, ScalarType::Integer(integer)
                        if Ok(integer) == semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned, 64))
                }
                StructuralFieldType::Structural(child)
                    if find_structural_type(declarations, child).is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ElementView { .. }
                                | StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                                        access: Some(terminal_psi::StructuralAccess::SharedBorrow,),
                                    },
                                )
                        )
                    }) =>
                {
                    // An element-view member spells its shared loan through
                    // its structural type: same two-word descriptor, extent
                    // word after the pointer.
                    length_word = 8;
                    observes_byte_length
                        && matches!(scalar, ScalarType::Integer(integer)
                        if Ok(integer) == semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned, 64))
                }
                StructuralFieldType::Scalar(actual) | StructuralFieldType::Atomic(actual) => {
                    !observes_byte_length && actual == scalar
                }
                StructuralFieldType::IeeeFloat(format) => {
                    !observes_byte_length && ScalarType::IeeeFloat(format) == scalar
                }
                StructuralFieldType::BoundedInteger(bounds) => {
                    !observes_byte_length && ScalarType::Integer(bounds.integer_type()) == scalar
                }
                StructuralFieldType::RefinedScalar(ref refinement) => {
                    !observes_byte_length && refinement.carrier() == scalar
                }
                _ => false,
            };
            if !matches_type {
                return None;
            }
            let offset = carrier_offset
                .checked_add(offset)?
                .checked_add(length_word)?;
            let bytes = if let Some(stored) = candidate
                .storage
                .as_ref()
                .and_then(|storage| storage.stored_scalar())
            {
                u32::from(runtime_abi::ScalarStorageCodec::from_scalar(stored)?.byte_size())
            } else {
                scalar_shape(scalar)?.byte_size
            };
            // A view root's descriptor is not the write's frame: the
            // referent's extent bounds the store, so the descriptor-size
            // ceiling applies only to storage the path stays inside.
            if !scalar_access_within_storage(root, offset, bytes, &elements, declarations) {
                return None;
            }
            return Some((offset, bytes, elements));
        }
        offset = offset.checked_add(layout.byte_size)?;
    }
    None
}

/// Bound full physical accesses, including fragmented transfer containers.
/// A view's descriptor extent is not its payload bound; dynamic payload
/// accesses retain their admitted element obligations instead.
fn scalar_access_within_storage(
    root: StructuralTypeId,
    offset: u32,
    bytes: u32,
    elements: &[RuntimeElement],
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let descriptor_frame = matches!(
        find_structural_type(declarations, root).map(|declaration| &declaration.shape),
        Some(
            StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
        )
    );
    descriptor_frame
        || shape(root, declarations).is_some_and(|storage| {
            runtime_span_within_storage(
                u64::from(offset),
                u64::from(bytes),
                elements,
                u64::from(storage.byte_size),
            )
        })
}

/// Exact static carrier geometry: record fields and bounded fixed-array indices.
/// Callers independently reconstruct readable root custody and availability;
/// this helper does not grant access to storage.
pub(crate) fn field_read(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32)> {
    if !path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    // An element view is a descriptor, not the observed storage: its live
    // extent — not a root aggregate footprint — bounds the read. Every inline
    // carrier uses its declared layout, including a mixed shape's common
    // fields beside its tag. Requiring a record here would reject that layout
    // before the exact field projection below can even identify the leaf.
    let root_shape = &find_structural_type(declarations, structural_type)?.shape;
    let root = match root_shape {
        StructuralTypeShape::ElementView { .. } => None,
        _ => Some(shape(structural_type, declarations)?),
    };
    if let Some(read) = mixed_common_field_read(structural_type, path, field, scalar, declarations)
    {
        return (root.is_none()
            || u64::from(read.0).checked_add(u64::from(read.1))?
                <= u64::from(root.unwrap().byte_size))
        .then_some(read);
    }
    scalar_field_geometry(structural_type, path, field, scalar, declarations, false)
        .map(|(offset, bytes, _)| (offset, bytes))
}

/// A mixed shape's common field sits on the carrier beside the tag whatever
/// case is active, at its offset among the conventional layout's common
/// fields: the layout mixed case establishment writes. `None` when the carrier
/// `path` reaches is not a mixed shape declaring this relevant scalar field.
fn mixed_common_field_read(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32)> {
    let (carrier, carrier_offset) = project(structural_type, path, declarations)?;
    let (offset, bytes) = mixed_common_field_geometry(carrier, field, scalar, declarations)?;
    Some((carrier_offset.checked_add(offset)?, bytes))
}

/// The offset within `carrier` and the width of its relevant scalar common
/// `field`. `None` when `carrier` is not a mixed shape declaring that field
/// with exactly this scalar type.
fn mixed_common_field_geometry(
    carrier: StructuralTypeId,
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32)> {
    let StructuralTypeShape::Mixed { fields, .. } =
        &find_structural_type(declarations, carrier)?.shape
    else {
        return None;
    };
    let (position, declaration) = fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
        .enumerate()
        .find(|(_, candidate)| candidate.id == field)?;
    if declaration.field_type.scalar_type() != Some(scalar) {
        return None;
    }
    let placed = case_layout(carrier, declarations)?
        .common_fields
        .get(position)
        .copied()?;
    let bytes = scalar_shape(scalar)?.byte_size;
    (placed.shape.byte_size == bytes).then_some((placed.byte_offset, bytes))
}

/// Case payload geometry for one scalar observation. `path` is the runtime
/// carrier to a sum, in the same bounded grammar `field_read` admits, and
/// `field` is a relevant payload field of the declared `case`. A payload
/// overlays the sum's own place, so the offset is the sum's offset plus the
/// field's offset in that case's conventional layout: the layout
/// `case_projection` copies and case establishment writes use. The case is a
/// verified claim about which overlay is live, not layout evidence; the field
/// is found only within that case, so a substituted case or field fails
/// instead of reading a neighbouring overlay.
pub(crate) fn case_field_read(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32)> {
    if path.iter().any(|segment| {
        matches!(
            segment,
            StructuralPathSegment::RuntimeIndex { .. }
                | StructuralPathSegment::Referent
                | StructuralPathSegment::FixedByteRange { .. }
        )
    }) {
        return None;
    }
    let root = shape(structural_type, declarations)?;
    let (sum, sum_offset) = project(structural_type, path, declarations)?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &find_structural_type(declarations, sum)?.shape
    else {
        return None;
    };
    let case_position = structural_case_position(cases, case)?;
    let (field_position, declaration) = cases[case_position]
        .fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
        .enumerate()
        .find(|(_, candidate)| candidate.id == field)?;
    if declaration.field_type.scalar_type() != Some(scalar) {
        return None;
    }
    let placed = case_layout(sum, declarations)?
        .cases
        .get(case_position)?
        .fields
        .get(field_position)
        .copied()?;
    let bytes = scalar_shape(scalar)?.byte_size;
    let offset = sum_offset.checked_add(placed.byte_offset)?;
    (placed.shape.byte_size == bytes
        && u64::from(offset).checked_add(u64::from(bytes))? <= u64::from(root.byte_size))
    .then_some((offset, bytes))
}

/// Exact byte metadata geometry at a projected record, including every
/// runtime selector retained by the common structural path owner. The
/// carrier distinguishes descriptor storage from inline owned byte payload.
pub(crate) struct ProjectedByteField {
    pub length_offset: u32,
    pub descriptor_offset: Option<u32>,
    pub capacity: Option<u64>,
    pub elements: Vec<RuntimeElement>,
    pub pointer_slots: Vec<PointerSlot>,
}

pub(crate) fn projected_byte_field(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ProjectedByteField> {
    let selected =
        terminal_semantics::projected_field_declaration(declarations.iter(), root, path, field)?;
    let (capacity, length_word) = match selected.field_type.byte_sequence_carrier() {
        Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) => (Some(capacity), 0),
        Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }) => (None, 8),
        _ => match selected
            .field_type
            .as_structural()
            .and_then(|identity| find_structural_type(declarations, identity))
            .map(|declaration| &declaration.shape)
        {
            Some(
                StructuralTypeShape::ElementView { .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    },
                ),
            ) => (None, 8),
            _ => return None,
        },
    };
    // Resolve the complete field occurrence through the common storage walker.
    // A trailing Case is a namespace, so it must stay joined to its Field.
    // Inline byte storage needs no invented standalone type declaration.
    let mut field_path = path.to_vec();
    field_path.push(StructuralPathSegment::Field(selected.identity));
    let regions = storage_projection_regions(root, &field_path, declarations)?;
    let (_, offset, leaf_elements) = project_storage(
        regions.root,
        field_path[regions.segment_start..]
            .iter()
            .map(Projection::from),
        declarations,
    )?;
    let mut elements = regions.elements;
    elements.extend(leaf_elements);
    Some(ProjectedByteField {
        length_offset: offset.checked_add(length_word)?,
        descriptor_offset: capacity.is_none().then_some(offset),
        capacity,
        elements,
        pointer_slots: regions.pointer_slots,
    })
}

/// The two-word descriptor slot one shared-view store replaces. A field names
/// that view member of the record at `path`; no field with an empty path names
/// the whole root, a descriptor home whose pointer word sits at offset zero
/// and whose extent word follows it.
pub(crate) fn projected_view_slot(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ProjectedByteField> {
    let Some(field) = field else {
        let declaration = find_structural_type(declarations, root)?;
        return (path.is_empty()
            && matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
            ))
        .then_some(ProjectedByteField {
            length_offset: 8,
            descriptor_offset: Some(0),
            capacity: None,
            elements: Vec::new(),
            pointer_slots: Vec::new(),
        });
    };
    let geometry = projected_byte_field(root, path, field, declarations)?;
    geometry.descriptor_offset?;
    Some(geometry)
}

/// Bounded inline byte storage starts with an aligned u64 live length, followed
/// by capacity bytes. The enclosing record rounds the capacity+8 footprint for
/// its next field; a borrowed view's pointer/length descriptor is a different
/// carrier. This reconstructs mutation geometry only, never content authority.
pub(crate) fn byte_field_length(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u32)> {
    if !terminal_psi::is_static_structural_scalar_store_path(path) {
        return None;
    }
    // The selected carrier owns the field, not necessarily the root. The
    // common field geometry checks its declaration and bounds the complete
    // access against the enclosing home, including fixed-array projections.
    scalar_field_geometry(structural_type, path, field, scalar, declarations, true)
        .map(|(offset, bytes, _)| (offset, bytes))
}

/// Metadata and inline backing share a field, but capacity bounds the destination
/// only. A replacement's source-length obligation supplies its dynamic footprint.
pub(crate) fn byte_field_storage(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .ok()?,
    );
    let (offset, _) = byte_field_length(structural_type, path, field, scalar, declarations)?;
    let (carrier, _) = project(structural_type, path, declarations)?;
    let StructuralTypeShape::Record { fields, .. } =
        &find_structural_type(declarations, carrier)?.shape
    else {
        return None;
    };
    let mut matching = fields.iter().filter(|candidate| candidate.id == field);
    let selected = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) =
        selected.field_type.byte_sequence_carrier()
    else {
        return None;
    };
    (u64::from(offset).checked_add(8)?.checked_add(capacity)?
        <= u64::from(shape(structural_type, declarations)?.byte_size))
    .then_some((offset, capacity))
}

/// A typed representation loan keeps both declarations. Portable validity
/// grants no target layout authority: replay each realized leaf and padded
/// size independently before transporting the original backing pointer.
pub(crate) fn typed_collection_layout_matches(
    collection: &StructuralTypeShape,
    target: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let StructuralTypeShape::FixedArray {
        element: source, ..
    } = collection
    else {
        return true;
    };
    if *source == target {
        return true;
    }
    let Some(relation) = terminal_semantics::element_representation_relation(
        declarations.iter(),
        *source,
        target,
        access,
    ) else {
        return false;
    };
    [
        (*source, &relation.source_layout),
        (target, &relation.target_layout),
    ]
    .into_iter()
    .all(|(element, layout)| {
        shape(element, declarations).is_some_and(|shape| {
            u64::from(shape.byte_size) == layout.size
                && u64::from(shape.alignment) == layout.alignment
        }) && layout.leaves.iter().all(|leaf| {
            element_leaf_geometry(element, &leaf.path, leaf.scalar_type, declarations)
                .is_some_and(|(offset, _)| u64::from(offset) == leaf.offset)
        })
    })
}
