/*
We project retained native layouts into pointer/descriptor slots,
collection views (`structural_reference_input`), and unit parameter/result
contracts (`structural_unit_input`) shared by target legalization and
instruction selection.
*/

pub(crate) mod structural_reference_input {
    use crate::native_geometry::NativeGeometry;
    use calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement, ValueShape};
    use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
        structural_case_position,
    };

    fn find_structural_type<'a>(
        declarations: &NativeGeometry<'a>,
        root: StructuralTypeId,
    ) -> Option<&'a StructuralTypeDeclaration> {
        declarations.declaration(root).ok()
    }

    pub(crate) fn scalar_storage_type(
        root: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ScalarType> {
        let StructuralTypeShape::PrimitiveScalar(scalar) =
            find_structural_type(declarations, root)?.shape
        else {
            return None;
        };
        runtime_abi::ScalarStorageCodec::from_scalar(scalar).map(|_| scalar)
    }

    pub(crate) fn scalar_reference_export_width(
        selected: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
        declarations: &NativeGeometry<'_>,
    ) -> Option<u32> {
        let scalar = scalar_storage_type(selected, declarations)?;
        terminal_semantics::reference_storage_export_byte_width(scalar, access).map(u32::from)
    }

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
            ] if *byte_size == placement.shape.byte_size
                && *alignment == placement.shape.alignment =>
            {
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
        ValueShape::for_scalar(scalar)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RuntimeElement {
        pub index: semantic_vocabulary::ValueId,
        pub obligation: semantic_vocabulary::ObligationId,
        pub stride: u32,
        pub extent: Option<u64>,
    }

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

    enum RuntimeReach {
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

    #[derive(Debug, PartialEq, Eq)]
    pub struct PointerSlot {
        pub byte_offset: u32,
        pub indices: std::ops::Range<usize>,
        pub scalar_descriptor: bool,
    }

    pub struct ReferenceSourceGeometry<Endpoint = StructuralTypeId> {
        pub region_root: StructuralTypeId,
        pub uses_borrowed_backing: bool,
        pub readable: bool,
        pub selected: Endpoint,
        pub byte_offset: u32,
        pub pointer_slots: Vec<PointerSlot>,
        pub elements: Vec<RuntimeElement>,
    }

    pub(crate) fn element_view_length_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
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
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceSourceGeometry> {
        reference_source_geometry_with_leaf_type(root, path, declarations, descriptor_leaf_type)
    }

    pub fn reference_formation_geometry<'types>(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'types>,
    ) -> Option<ReferenceSourceGeometry<std::borrow::Cow<'types, StructuralFieldType>>> {
        reference_geometry(root, path, declarations, |root, path| {
            if let Some((StructuralPathSegment::FixedRange { .. }, prefix)) = path.split_last() {
                let (storage, _, elements) =
                    runtime_storage_projection(root, prefix, declarations)?;
                let region =
                    terminal_semantics::reference_storage_region(declarations.iter(), root, path)?;
                region.array?;
                Some((storage, u32::try_from(region.byte_offset).ok()?, elements))
            } else {
                runtime_storage_projection(root, path, declarations)
            }
        })
    }

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
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceSourceGeometry> {
        reference_source_geometry_with_leaf_type(
            root,
            path,
            declarations,
            StructuralFieldType::canonical_leaf_type,
        )
    }

    pub(crate) fn storage_address_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceSourceGeometry<()>> {
        reference_geometry(root, path, declarations, |root, path| {
            let (_, offset, elements) = runtime_storage_projection(root, path, declarations)?;
            Some(((), offset, elements))
        })
    }

    pub(crate) fn owner_loan_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceSourceGeometry> {
        reference_source_geometry_with_leaf_type(root, path, declarations, descriptor_leaf_type)
    }

    pub(crate) fn leaf_copy_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        expected: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceSourceGeometry> {
        let expected = declarations.iter().find(|row| row.id == expected)?;
        reference_source_geometry_with_leaf_type(root, path, declarations, |field| {
            field.leaf_copy_type(Some(expected))
        })
    }

    fn reference_source_geometry_with_leaf_type(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
        leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType> + Copy,
    ) -> Option<ReferenceSourceGeometry> {
        reference_geometry(root, path, declarations, |root, path| {
            runtime_projection_with_leaf_type(root, path, declarations, leaf_type)
        })
    }

    fn reference_geometry<Endpoint>(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
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
            uses_borrowed_backing: regions.uses_borrowed_backing,
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

    fn storage_projection_regions(
        mut root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
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
                        let scalar_descriptor =
                            scalar_storage_type(*referent, declarations).is_some();
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
                    ) if borrowed.is_borrowed_byte_carrier() => (
                        carrier,
                        position,
                        false,
                        shape(carrier, declarations)?.byte_size,
                    ),
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

    pub(crate) struct PrimitiveAccessGeometry {
        pub(crate) byte_offset: u32,
        pub(crate) byte_size: u32,
        pub(crate) elements: Vec<RuntimeElement>,
        pub(crate) pointer_slots: Vec<PointerSlot>,
        pub(crate) uses_borrowed_backing: bool,
    }

    pub(crate) fn atomic_cell_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<PrimitiveAccessGeometry> {
        terminal_semantics::atomic_cell_type(declarations.iter(), root, path)
            .filter(|cell| cell.scalar_type() == scalar)?;
        let regions = storage_projection_regions(root, path, declarations)?;
        if !regions.readable || !regions.elements.is_empty() {
            return None;
        }
        let suffix = &path[regions.segment_start..];
        let (byte_offset, byte_size, elements) =
            if let Some((StructuralPathSegment::Field(_), prefix)) = suffix.split_last() {
                let field = terminal_semantics::runtime_structural_field(
                    declarations.iter(),
                    regions.root,
                    suffix,
                )?;
                store(regions.root, prefix, field.id, scalar, declarations)?
            } else {
                let (selected, offset, elements) =
                    runtime_projection(regions.root, suffix, declarations)?;
                if !matches!(find_structural_type(declarations, selected)?.shape,
                StructuralTypeShape::AtomicCell(cell) if cell.scalar_type() == scalar)
                {
                    return None;
                }
                (offset, shape(selected, declarations)?.byte_size, elements)
            };
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

    pub(crate) fn primitive_store(
        destination: &abstract_operations::StructuralScalarStoreDestination,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
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
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::StructuralAccess::MutableBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                ) {
                    return None;
                }
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                result, ..
            } => {
                if !result.qualification_establishments.is_empty() || !result.claims.is_empty() {
                    return None;
                }
            }
            abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                parameter,
                ..
            } => {
                if parameter.access != terminal_psi::StructuralAccess::Owned {
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

    fn primitive_access_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
        endpoint: impl Fn(
            StructuralTypeId,
            &[StructuralPathSegment],
            ScalarType,
            &NativeGeometry<'_>,
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

    pub(crate) fn primitive_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
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
            let (payload_offset, extent) = match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    (8u32, Some(capacity))
                }
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
        if matches.next().is_some()
            || declaration.shape != StructuralTypeShape::PrimitiveScalar(scalar)
        {
            return None;
        }
        let bytes = scalar_shape(scalar)?.byte_size;
        scalar_access_within_storage(root, offset, bytes, &elements, declarations)
            .then_some((offset, bytes, elements))
    }

    pub(crate) fn primitive_read_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
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
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32, Vec<RuntimeElement>)> {
        if let Some(geometry) = primitive_geometry(root, path, scalar, declarations) {
            return Some(geometry);
        }
        let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
            return None;
        };
        let field = terminal_semantics::runtime_structural_field(declarations.iter(), root, path)?;
        if field.field_type.scalar_type() != Some(scalar) {
            return None;
        }
        inline_scalar_field_geometry(root, carrier, field.id, scalar, declarations)
    }

    pub(crate) fn shape(
        root: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ValueShape> {
        declarations.shape(root).ok()
    }

    pub(crate) fn parameter_shape<'declaration>(
        parameter: impl Into<abstract_operations::StructuralCallFormal<'declaration>>,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ValueShape> {
        use terminal_psi::StructuralAccess;
        let parameter = parameter.into();
        if parameter.access == StructuralAccess::Owned {
            return stored_value_shape(parameter.structural_type, declarations);
        }
        match (parameter.access, parameter.multiplicity) {
            (
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
                terminal_psi::nonlinear_multiplicity_pair_pattern!(),
            ) => internal_borrow_shape(parameter.structural_type, declarations),
            _ => None,
        }
    }

    pub(crate) fn internal_borrow_shape(
        structural_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn owned_storage_shape(
        structural_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ValueShape> {
        find_structural_type(declarations, structural_type).filter(|declaration| {
            declaration.shape.is_composite_shape()
                || declaration.shape.is_bounded_owned_byte_sequence()
        })?;
        stored_value_shape(structural_type, declarations)
    }

    pub(crate) fn stored_value_shape(
        structural_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ValueShape> {
        shape(structural_type, declarations)
    }

    pub(crate) fn primitive_array_shape(
        root: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ValueShape> {
        let mut current = root;
        let mut array = false;
        for _ in 0..declarations.len() {
            match find_structural_type(declarations, current)?.shape {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(_),
                } => {
                    array = true;
                    current = element;
                }
                StructuralTypeShape::PrimitiveScalar(scalar) if array => {
                    if matches!(scalar, ScalarType::Integer(integer)
                        if integer.carrier() != semantic_vocabulary::IntegerCarrier::Fixed)
                    {
                        return None;
                    }
                    // Scalar operation eligibility is narrower than storage layout.
                    scalar_shape(scalar)?;
                    return shape(root, declarations);
                }
                _ => return None,
            }
        }
        None
    }

    pub(crate) fn borrowed_byte_view_presentation(
        source: &terminal_psi::StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        view_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, Option<u64>, Option<u32>)> {
        if !declarations.iter().any(|declaration| {
            declaration.id == view_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    )
                )
        }) {
            return None;
        }
        if let Some((offset, length)) =
            fixed_byte_array_view(source, argument, view_type, declarations)
        {
            return Some((offset, Some(length), Some(1)));
        }
        bounded_byte_view(source, argument, view_type, declarations)
            .filter(|geometry| geometry.elements.is_empty())
            .map(|geometry| (geometry.offset, None, None))
    }

    pub(crate) fn fixed_byte_array_view(
        source: &terminal_psi::StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        view_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u64)> {
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
        if terminal_semantics::borrowed_referent_presentation(
            declarations.iter(),
            backing,
            view_type,
        ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
        {
            return None;
        }
        let root = shape(source.structural_type, declarations)?;
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

    pub(crate) fn fixed_element_array_view(
        source: &terminal_psi::StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        view_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u64, u32)> {
        let geometry = collection_view_geometry(source, argument, view_type, declarations)?;
        let terminal_semantics::CollectionExtent::Fixed(length) = geometry.extent else {
            return None;
        };
        (geometry.storage == terminal_semantics::CollectionStorage::Inline
            && geometry.elements.is_empty())
        .then_some((geometry.offset, length, geometry.stride))
    }

    pub(crate) mod collection_views {
        use super::*;

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
            declarations: &NativeGeometry<'types>,
        ) -> Option<ProjectedCollection<'types>> {
            let regions = storage_projection_regions(root, path, declarations)?;
            if !regions.readable {
                return None;
            }
            let leaf_path = &path[regions.segment_start..];
            let (shape, offset, leaf_elements) =
                collection_projection(regions.root, leaf_path, declarations)?;
            let (storage, _, _) =
                runtime_storage_projection(regions.root, leaf_path, declarations)?;
            let byte_size = field_shape(&storage, declarations, root)?.byte_size;
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
            pub length_scale: u64,
            pub storage: terminal_semantics::CollectionStorage,
        }

        pub(crate) fn collection_view_geometry(
            source: &terminal_psi::StructuralParameterDeclaration,
            argument: &terminal_psi::StructuralArgument,
            view_type: StructuralTypeId,
            declarations: &NativeGeometry<'_>,
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
            declarations: &NativeGeometry<'_>,
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
                        StructuralAccess::SharedBorrow
                            | terminal_psi::mutating_access_pair_pattern!(),
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
            let (field_path, byte_window) = match argument.path.last() {
                Some(StructuralPathSegment::FixedRange { start, end }) => (
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
            let collection =
                projected_collection(source.structural_type, field_path, declarations)?;
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
            let mut representation_source_stride = None;
            let mut length_scale = 1;
            let (backing, representation) = match geometry {
                terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing) => {
                    (backing, None)
                }
                terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                    backing,
                    source: source_layout,
                    target: target_layout,
                } => {
                    if byte_window.is_some() || argument.access != StructuralAccess::SharedBorrow {
                        return None;
                    }
                    length_scale = whole_representation_length_scale(
                        &collection_shape,
                        &source_layout,
                        element,
                        &target_layout,
                        declarations,
                    )?;
                    match backing.extent {
                        terminal_semantics::CollectionExtent::Fixed(count) => {
                            count.checked_mul(length_scale)?;
                        }
                        terminal_semantics::CollectionExtent::StoredLength => {}
                        terminal_semantics::CollectionExtent::LiveLength { .. } => return None,
                    }
                    representation_source_stride = Some(source_layout.size);
                    (backing, Some(target_layout))
                }
                terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
                    byte_extent,
                    element: layout,
                } => {
                    if byte_window.is_some() {
                        return None;
                    }
                    let storage =
                        if matches!(&*collection_shape, StructuralTypeShape::ByteSequence(_)) {
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
            if let Some(layout) = &representation
                && (layout.size != u64::from(element_stride)
                    || layout.alignment != u64::from(shape(element, declarations)?.alignment)
                    || layout.leaves.iter().any(|leaf| {
                        element_leaf_geometry(element, &leaf.path, leaf.scalar_type, declarations)
                            .is_none_or(|(offset, _)| u64::from(offset) != leaf.offset)
                    }))
            {
                return None;
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
                (Some(_), _) => return None,
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
                (None, terminal_semantics::CollectionExtent::StoredLength) => {
                    (0, 0, terminal_semantics::CollectionExtent::StoredLength)
                }
            };
            let offset = offset.checked_add(window_offset)?;
            let storage_size = match backing.storage {
                terminal_semantics::CollectionStorage::Inline => {
                    if let Some(source_stride) = representation_source_stride {
                        element_count.checked_mul(source_stride)?
                    } else if representation.is_some() {
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
                length_scale,
                storage: backing.storage,
            })
        }

        pub(crate) fn whole_representation_length_scale(
            collection: &StructuralTypeShape,
            source_layout: &terminal_semantics::RepresentationElementLayout,
            target: StructuralTypeId,
            target_layout: &terminal_semantics::RepresentationElementLayout,
            declarations: &NativeGeometry<'_>,
        ) -> Option<u64> {
            let source = match collection {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. } => *element,
                _ => return None,
            };
            if source_layout.size == 0
                || target_layout.size == 0
                || !source_layout.size.is_multiple_of(target_layout.size)
                || source_layout.alignment < target_layout.alignment
            {
                return None;
            }
            for (element, layout) in [(source, source_layout), (target, target_layout)] {
                let realized = shape(element, declarations)?;
                if u64::from(realized.byte_size) != layout.size
                    || u64::from(realized.alignment) != layout.alignment
                    || layout.leaves.iter().any(|leaf| {
                        element_leaf_geometry(element, &leaf.path, leaf.scalar_type, declarations)
                            .is_none_or(|(offset, _)| u64::from(offset) != leaf.offset)
                    })
                {
                    return None;
                }
            }
            Some(source_layout.size / target_layout.size)
        }
    }
    pub(crate) use collection_views::{
        collection_view_geometry, collection_view_region_geometry, projected_collection,
        whole_representation_length_scale,
    };

    pub(crate) struct BoundedByteViewGeometry {
        pub(crate) offset: u32,
        pub(crate) capacity: u64,
        pub(crate) elements: Vec<RuntimeElement>,
    }

    pub(crate) fn bounded_byte_view(
        source: &terminal_psi::StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        view_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<BoundedByteViewGeometry> {
        if source.place != argument.place
            || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
            || !source.projected_qualifications.is_empty()
        {
            return None;
        }
        if argument.access != terminal_psi::StructuralAccess::SharedBorrow {
            return bounded_byte_field_borrow(source, argument, view_type, declarations, false)
                .map(|(offset, capacity)| BoundedByteViewGeometry {
                    offset,
                    capacity,
                    elements: Vec::new(),
                });
        }
        let expected = terminal_psi::StructuralParameterDeclaration {
            place: argument.place,
            position: 0,
            structural_type: view_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        };
        let capacity = terminal_semantics::shared_boundary_buffer_capacity(
            declarations.iter(),
            source.structural_type,
            argument,
            &expected,
        )?;
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
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u64)> {
        bounded_byte_field_borrow(source, argument, view_type, declarations, true)
    }

    fn bounded_byte_field_borrow(
        source: &terminal_psi::StructuralParameterDeclaration,
        argument: &terminal_psi::StructuralArgument,
        view_type: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn projected_scalar_codec(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
    ) -> Option<runtime_abi::ScalarStorageCodec> {
        let (root, path) = referent_projection(root, path, declarations)?;
        if let Some((StructuralPathSegment::Field(_), prefix)) = path.split_last() {
            let field =
                terminal_semantics::runtime_structural_field(declarations.iter(), root, path)?;
            return scalar_field_codec(root, prefix, field.id, declarations);
        }
        let (storage, _, _) = runtime_storage_projection(root, path, declarations)?;
        let scalar = match &*storage {
            StructuralFieldType::Scalar(scalar) => *scalar,
            StructuralFieldType::Structural(selected) => {
                scalar_storage_type(*selected, declarations)?
            }
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

    pub(crate) fn projected_scalar_fragments<'declarations>(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &'declarations NativeGeometry<'_>,
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
        declarations: &NativeGeometry<'_>,
    ) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
        let regions = storage_projection_regions(root, path, declarations)?;
        Some((regions.root, &path[regions.segment_start..]))
    }

    pub(crate) fn scalar_field_codec(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn scalar_field_fragments<'declarations>(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        declarations: &'declarations NativeGeometry<'_>,
    ) -> Option<&'declarations semantic_vocabulary::AuthoredBitFragments> {
        let field = scalar_field_declaration(root, path, field, declarations)?;
        if matches!(
            field.field_type,
            StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
        ) {
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
        declarations: &'declarations NativeGeometry<'_>,
    ) -> Option<&'declarations terminal_psi::StructuralFieldDeclaration> {
        let (root, path) = referent_projection(root, path, declarations)?;
        terminal_semantics::projected_field_declaration(declarations.iter(), root, path, field)
    }

    pub(crate) fn field_shape(
        field: &StructuralFieldType,
        declarations: &NativeGeometry<'_>,
        owner: StructuralTypeId,
    ) -> Option<ValueShape> {
        declarations.field_shape(field, owner).ok()
    }

    pub(crate) fn field_storage_shape(
        field: &terminal_psi::StructuralFieldDeclaration,
        declarations: &NativeGeometry<'_>,
        owner: StructuralTypeId,
    ) -> Option<ValueShape> {
        declarations.field_storage_shape(field, owner).ok()
    }

    pub(crate) fn case_projection(
        root: StructuralTypeId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceSourceGeometry> {
        let endpoint =
            terminal_semantics::canonical_case_leaf_type(declarations.iter(), root, path)?;
        let storage_path =
            terminal_semantics::canonical_structural_storage_path(declarations.iter(), root, path)?;
        let geometry = leaf_copy_geometry(root, &storage_path, endpoint, declarations)?;
        (geometry.selected == endpoint
            && geometry.readable
            && geometry.elements.is_empty()
            && geometry
                .pointer_slots
                .iter()
                .all(|slot| !slot.scalar_descriptor))
        .then_some(geometry)
    }

    pub(crate) fn case_layout(
        root: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<calling_conventions::ConventionalSumLayout> {
        declarations.sum_layout(root).ok()
    }

    pub(crate) struct ReferenceProjection {
        pub(crate) selected: StructuralTypeId,
        pub(crate) pointer_slots: Vec<PointerSlot>,
        pub(crate) elements: Vec<RuntimeElement>,
        pub(crate) referent_offset: u32,
        pub(crate) scalar_descriptor: bool,
    }

    pub(crate) fn reference_projection(
        mut root: StructuralTypeId,
        mut path: &[StructuralPathSegment],
        access: terminal_psi::StructuralAccess,
        expected: Option<StructuralTypeId>,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ReferenceProjection> {
        use terminal_psi::StructuralAccess;
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
        let geometry =
            reference_geometry(initial_root, initial_path, declarations, |root, path| {
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
        declarations: &NativeGeometry<'_>,
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
        move |field| {
            if let Some(expected) = expected
                && let Some(
                    carrier @ (terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(granted),
                    }
                    | terminal_psi::ByteSequenceCarrier::BoundedView {
                        access: Some(granted),
                        ..
                    }),
                ) = field.byte_sequence_carrier()
            {
                return terminal_semantics::descriptor_reborrow_shapes_match(
                    &StructuralTypeShape::ByteSequence(carrier),
                    granted,
                    None,
                    &expected.shape,
                    access,
                )
                .then_some(StructuralFieldType::Structural(expected.id));
            }
            terminal_semantics::borrowed_field_presentation(field, access, expected)
        }
    }

    pub(crate) fn leaf_copy_projection(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        expected: StructuralTypeId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(StructuralTypeId, u32)> {
        let expected = declarations.iter().find(|row| row.id == expected)?;
        project_with_leaf_type(root, path, declarations, |field| {
            field.leaf_copy_type(Some(expected))
        })
    }

    fn project_with_leaf_type(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
        leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType> + Copy,
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

    pub(crate) fn collection_projection<'types>(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'types>,
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

    pub(crate) fn runtime_projection_elements(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
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
        declarations: &NativeGeometry<'_>,
        leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType> + Copy,
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

    fn project_storage<'types>(
        root: StructuralTypeId,
        path: impl Iterator<Item = Projection>,
        declarations: &NativeGeometry<'types>,
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
                        StructuralTypeShape::FixedArray {
                            element,
                            length: terminal_psi::ArrayExtent::Literal(length),
                        } => (*element, Some(*length)),
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
                        Projection::RuntimeIndex(index, obligation) => {
                            elements.push(RuntimeElement {
                                index,
                                obligation,
                                stride,
                                extent,
                            })
                        }
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
                        let placement = declarations
                            .field_placement(field, carrier, field_offset)
                            .ok()?;
                        let layout = placement.shape;
                        field_offset = placement.byte_offset;
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

    pub(crate) fn runtime_projection(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
    ) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
        runtime_projection_with_leaf_type(
            root,
            path,
            declarations,
            StructuralFieldType::canonical_leaf_type,
        )
    }

    pub(crate) fn runtime_borrowed_projection(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        access: terminal_psi::StructuralAccess,
        expected: Option<StructuralTypeId>,
        declarations: &NativeGeometry<'_>,
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
        let selected = expected
            .filter(|expected| {
                terminal_semantics::borrowed_referent_presentation(
                    declarations.iter(),
                    geometry.selected,
                    expected.id,
                ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
            })
            .map_or(geometry.selected, |expected| expected.id);
        Some((selected, geometry.byte_offset, geometry.elements))
    }

    fn runtime_projection_with_leaf_type(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'_>,
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

    fn runtime_storage_projection<'types>(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        declarations: &NativeGeometry<'types>,
    ) -> Option<(
        std::borrow::Cow<'types, StructuralFieldType>,
        u32,
        Vec<RuntimeElement>,
    )> {
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
        project_storage(root, path.iter().map(Projection::from), declarations)
    }

    pub(crate) fn field_store_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<PrimitiveAccessGeometry> {
        primitive_access_geometry(
            root,
            path,
            scalar,
            declarations,
            |root, path, scalar, declarations| {
                inline_scalar_field_geometry(root, path, field, scalar, declarations)
            },
        )
    }

    pub(crate) fn store(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32, Vec<RuntimeElement>)> {
        let geometry = field_store_geometry(root, path, field, scalar, declarations)?;
        Some((geometry.byte_offset, geometry.byte_size, geometry.elements))
    }

    fn inline_scalar_field_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32, Vec<RuntimeElement>)> {
        if let Some((StructuralPathSegment::Case(case), prefix)) = path.split_last() {
            let (carrier, carrier_offset, elements) =
                runtime_projection(root, prefix, declarations)?;
            let (offset, bytes) =
                case_field_read(carrier, &[], *case, field, scalar, declarations)?;
            return Some((carrier_offset.checked_add(offset)?, bytes, elements));
        }
        if let Some((carrier, carrier_offset, elements)) =
            runtime_projection(root, path, declarations)
            && let Some((offset, bytes)) =
                mixed_common_field_geometry(carrier, field, scalar, declarations)
        {
            return Some((carrier_offset.checked_add(offset)?, bytes, elements));
        }
        scalar_field_geometry(root, path, field, scalar, declarations, false)
    }

    pub(crate) fn element_leaf_geometry(
        element: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32)> {
        let (offset, bytes, elements) =
            element_leaf_projection(element, path, scalar, declarations)?;
        elements.is_empty().then_some((offset, bytes))
    }

    pub(crate) fn element_leaf_projection(
        element: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32, Vec<RuntimeElement>)> {
        if terminal_semantics::element_view_leaf_scalar(declarations, element, path) != Some(scalar)
        {
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
                return scalar_field_geometry(
                    element,
                    prefix,
                    field.id,
                    scalar,
                    declarations,
                    false,
                );
            }
        }
        primitive_geometry(element, path, scalar, declarations)
    }

    fn scalar_field_geometry(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
        observes_byte_length: bool,
    ) -> Option<(u32, u32, Vec<RuntimeElement>)> {
        let (carrier, carrier_offset, elements) = runtime_projection(root, path, declarations)?;
        if matches!(
            find_structural_type(declarations, carrier)?.shape,
            StructuralTypeShape::Mixed { .. }
        ) {
            if observes_byte_length {
                return None;
            }
            let (offset, bytes) =
                mixed_common_field_geometry(carrier, field, scalar, declarations)?;
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
                    || matches!(
                        candidate.field_type,
                        StructuralFieldType::Atomic(_)
                            | StructuralFieldType::AtomicBoundedInteger(_)
                    )
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
            let placement = declarations
                .field_placement(candidate, carrier, offset)
                .ok()?;
            let layout = placement.shape;
            offset = placement.byte_offset;
            if candidate.id == field {
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
                        if find_structural_type(declarations, child).is_some_and(
                            |declaration| {
                                matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ElementView { .. }
                                        | StructuralTypeShape::ByteSequence(
                                            terminal_psi::ByteSequenceCarrier::BorrowedView {
                                                access: Some(
                                                    terminal_psi::StructuralAccess::SharedBorrow,
                                                ),
                                            },
                                        )
                                )
                            },
                        ) =>
                    {
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
                    StructuralFieldType::BoundedInteger(bounds)
                    | StructuralFieldType::AtomicBoundedInteger(bounds) => {
                        !observes_byte_length
                            && ScalarType::Integer(bounds.integer_type()) == scalar
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
                if !scalar_access_within_storage(root, offset, bytes, &elements, declarations) {
                    return None;
                }
                return Some((offset, bytes, elements));
            }
            offset = offset.checked_add(layout.byte_size)?;
        }
        None
    }

    fn scalar_access_within_storage(
        root: StructuralTypeId,
        offset: u32,
        bytes: u32,
        elements: &[RuntimeElement],
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn field_read(
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32)> {
        if !path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Case(_)
            )
        }) {
            return None;
        }
        let root_shape = &find_structural_type(declarations, structural_type)?.shape;
        let root = match root_shape {
            StructuralTypeShape::ElementView { .. } => None,
            _ => Some(shape(structural_type, declarations)?),
        };
        if let Some(read) =
            mixed_common_field_read(structural_type, path, field, scalar, declarations)
        {
            return (root.is_none()
                || u64::from(read.0).checked_add(u64::from(read.1))?
                    <= u64::from(root.unwrap().byte_size))
            .then_some(read);
        }
        let (offset, bytes, elements) =
            scalar_field_geometry(structural_type, path, field, scalar, declarations, false)?;
        elements.is_empty().then_some((offset, bytes))
    }

    fn mixed_common_field_read(
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32)> {
        let (carrier, carrier_offset) = project(structural_type, path, declarations)?;
        let (offset, bytes) = mixed_common_field_geometry(carrier, field, scalar, declarations)?;
        Some((carrier_offset.checked_add(offset)?, bytes))
    }

    fn mixed_common_field_geometry(
        carrier: StructuralTypeId,
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn case_field_read(
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        case: semantic_vocabulary::StructuralCaseId,
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32)> {
        if path.iter().any(|segment| {
            matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::Referent
                    | StructuralPathSegment::FixedRange { .. }
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

    pub struct ProjectedByteField {
        pub length_offset: u32,
        pub descriptor_offset: Option<u32>,
        pub capacity: Option<u64>,
        pub elements: Vec<RuntimeElement>,
        pub pointer_slots: Vec<PointerSlot>,
    }

    pub fn projected_byte_field(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        declarations: &NativeGeometry<'_>,
    ) -> Option<ProjectedByteField> {
        let selected = terminal_semantics::projected_field_declaration(
            declarations.iter(),
            root,
            path,
            field,
        )?;
        let (capacity, length_word) = match selected.field_type.byte_sequence_carrier() {
            Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) => {
                (Some(capacity), 0)
            }
            Some(terminal_psi::ByteSequenceCarrier::BorrowedView { .. }) => (None, 8),
            _ => match selected
                .field_type
                .as_structural()
                .and_then(|identity| find_structural_type(declarations, identity))
                .map(|declaration| &declaration.shape)
            {
                Some(
                    StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                    ),
                ) => (None, 8),
                _ => return None,
            },
        };
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

    pub(crate) fn projected_view_slot(
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: Option<StructuralFieldId>,
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn byte_field_length(
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        scalar: ScalarType,
        declarations: &NativeGeometry<'_>,
    ) -> Option<(u32, u32)> {
        if !terminal_psi::is_static_structural_scalar_store_path(path) {
            return None;
        }
        scalar_field_geometry(structural_type, path, field, scalar, declarations, true)
            .map(|(offset, bytes, _)| (offset, bytes))
    }

    pub(crate) fn byte_field_storage(
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        field: StructuralFieldId,
        declarations: &NativeGeometry<'_>,
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

    pub(crate) fn typed_collection_layout_matches(
        collection: &StructuralTypeShape,
        target: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
        declarations: &NativeGeometry<'_>,
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
}
pub(crate) mod structural_unit_input {
    use crate::structural_inputs::structural_reference_input::{
        parameter_shape, stored_value_shape,
    };
    use calling_conventions::{
        CallPlan, IndirectPointerLocation, ValueClass, ValueLocation, ValuePlacement,
    };
    use terminal_psi::StructuralAccess;

    #[derive(Clone, Copy)]
    pub(crate) struct Parameter<'a> {
        pub semantic: &'a terminal_psi::StructuralParameterDeclaration,
        pub target: &'a target_operations::TargetStructuralParameter,
    }

    pub(crate) fn parameter_views(
        contract: &legalized_operations::LegalizedStructuralContract,
    ) -> Vec<Parameter<'_>> {
        contract
            .parameters
            .iter()
            .map(|parameter| Parameter {
                semantic: &parameter.semantic,
                target: &parameter.target,
            })
            .collect()
    }

    pub(crate) fn accepts_graph(
        call_plan: &CallPlan,
        parameters: &[Parameter<'_>],
        structural_types: &crate::native_geometry::NativeGeometry<'_>,
    ) -> bool {
        let Some(scalar_count) = call_plan.parameters.len().checked_sub(parameters.len()) else {
            return false;
        };
        let mut shapes = call_plan.parameters[..scalar_count]
            .iter()
            .map(|placement| placement.shape)
            .collect::<Vec<_>>();
        for (position, parameter) in parameters.iter().enumerate() {
            let semantic = parameter.semantic;
            let Some(shape) = (if semantic.access == StructuralAccess::Owned {
                stored_value_shape(semantic.structural_type, structural_types)
            } else {
                parameter_shape(semantic, structural_types)
            }) else {
                return false;
            };
            if semantic.position as usize != position
                || parameters[..position]
                    .iter()
                    .any(|previous| previous.semantic.place == semantic.place)
                || parameter.target.place != semantic.place
                || parameter.target.structural_type != semantic.structural_type
                || parameter.target.access != semantic.access
                || parameter.target.multiplicity != semantic.multiplicity
                || parameter.target.projected_qualifications != semantic.projected_qualifications
                || parameter.target.shape != shape
                || parameter.target.placement != call_plan.parameters[scalar_count + position]
            {
                return false;
            }
            shapes.push(shape);
        }
        calling_conventions::evaluate_call_plan(
            call_plan.policy,
            &calling_conventions::CallSignature {
                parameters: shapes,
                result: call_plan.result.as_ref().map(|placement| placement.shape),
            },
        )
        .is_ok_and(|expected| expected == *call_plan)
    }

    pub(crate) fn owned_indirect_pointer(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        placement: &ValuePlacement,
    ) -> Option<IndirectPointerLocation> {
        if parameter.access != StructuralAccess::Owned
            || placement.shape.class != ValueClass::Integer
            || placement.shape.byte_size == 0
        {
            return None;
        }
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: Some(_),
                byte_size,
                alignment,
            },
        ] = placement.locations.as_slice()
        else {
            return None;
        };
        (*byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment)
            .then_some(*pointer)
    }
}

pub(crate) use structural_reference_input::{parameter_shape, runtime_indices_match, shape};
