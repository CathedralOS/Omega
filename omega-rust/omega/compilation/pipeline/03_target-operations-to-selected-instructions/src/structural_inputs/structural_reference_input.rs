//! Input-only reconstruction of structural storage and borrowed-pointer geometry.
//! Whole owned aggregates and borrowed referents share recursive payload layout;
//! their callers independently check access, ownership and exact ABI placement.
use calling_conventions::{
    IndirectPointerLocation, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Locate pointer bits in an exact borrowed-reference ABI placement, not referent bytes.
pub(crate) fn stack_pointer_offset(placement: &ValuePlacement) -> Option<u32> {
    if placement.shape.class != ValueClass::BorrowedReference {
        return None;
    }
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
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
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
fn runtime_reach(elements: &[RuntimeElement]) -> Option<u64> {
    elements.iter().try_fold(0u64, |reach, element| {
        reach.checked_add(u64::from(element.stride).checked_mul(element.extent?.checked_sub(1)?)?)
    })
}

/// Reconstruct a primitive write without inventing a carrier record or field.
pub(crate) fn primitive_store(
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8, Vec<RuntimeElement>)> {
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
            if path.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
            {
                return None;
            }
        }
        // Only the scalar field store writes an owned block home natively.
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { .. } => {
            return None;
        }
    }
    primitive_geometry(destination.structural_type(), path, scalar, declarations)
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
) -> Option<(u32, u8, Vec<RuntimeElement>)> {
    let mut roots = declarations
        .iter()
        .filter(|declaration| declaration.id == root);
    let declaration = roots.next()?;
    if roots.next().is_some() {
        return None;
    }
    if let StructuralTypeShape::ByteSequence(carrier) = declaration.shape {
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
        return match path {
            [StructuralPathSegment::FixedIndex(index)] => {
                if extent.is_some_and(|extent| *index >= extent) {
                    return None;
                }
                Some((
                    payload_offset.checked_add(u32::try_from(*index).ok()?)?,
                    1,
                    Vec::new(),
                ))
            }
            [StructuralPathSegment::RuntimeIndex { index, obligation }] => Some((
                payload_offset,
                1,
                vec![RuntimeElement {
                    index: *index,
                    obligation: *obligation,
                    stride: 1,
                    extent,
                }],
            )),
            _ => None,
        };
    }
    if let Some((StructuralPathSegment::Field(identity), carrier_path)) = path.split_last() {
        let (carrier, _, _) = runtime_projection(root, carrier_path, declarations)?;
        if let StructuralTypeShape::Record { fields } = &declarations
            .iter()
            .find(|declaration| declaration.id == carrier)?
            .shape
        {
            let field = fields
                .iter()
                .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
            if matches!(
                field.field_type,
                StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
                    | StructuralFieldType::IeeeFloat(_)
            ) {
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
    let bytes = u8::try_from(scalar_shape(scalar)?.byte_size).ok()?;
    (u64::from(offset)
        .checked_add(runtime_reach(&elements)?)?
        .checked_add(u64::from(bytes))?
        <= u64::from(shape(root, declarations)?.byte_size))
    .then_some((offset, bytes, elements))
}

/// Reconstruct the leaf a primitive *read* observes: every leaf
/// `primitive_geometry` admits, plus a scalar field that ends a projection
/// selecting a record element at run time (`cells[i].hp`). The field is the
/// one `terminal_semantics::runtime_element_field` names from declarations,
/// and its geometry is the field store's: the carrier's static offset and
/// runtime elements, then the field's declared offset in the record. A store
/// never takes this route; its field write is `store`.
pub(crate) fn primitive_read_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8, Vec<RuntimeElement>)> {
    if let Some(geometry) = primitive_geometry(root, path, scalar, declarations) {
        return Some(geometry);
    }
    let (leaf, record) =
        terminal_semantics::runtime_element_field(declarations.iter(), root, path)?;
    let (StructuralPathSegment::Field(identity), carrier) = path.split_last()? else {
        return None;
    };
    let StructuralTypeShape::Record { fields } = &record.shape else {
        return None;
    };
    let field = fields.iter().find(|field| field.identity == *identity)?;
    if leaf != scalar {
        return None;
    }
    scalar_field_geometry(root, carrier, field.id, scalar, declarations, false)
}

pub(crate) fn shape(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    shape_inner(root, declarations, &mut Vec::new())
}

#[test]
fn reference_payload_is_one_address_word_per_leaf() {
    let referent = StructuralTypeId::new(9100).unwrap();
    let reference = StructuralTypeId::new(9101).unwrap();
    let array = StructuralTypeId::new(9102).unwrap();
    let declarations = [
        StructuralTypeDeclaration {
            id: referent,
            identity: "EmptyReferent".into(),
            shape: StructuralTypeShape::Record { fields: vec![] },
        },
        StructuralTypeDeclaration {
            id: reference,
            identity: "MutableReference".into(),
            shape: StructuralTypeShape::Reference {
                referent,
                access: terminal_psi::StructuralAccess::MutableBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "ReferencePair".into(),
            shape: StructuralTypeShape::FixedArray {
                element: reference,
                length: 2,
            },
        },
    ];
    assert_eq!(
        shape(referent, &declarations),
        Some(ValueShape::integer(0, 1))
    );
    assert_eq!(
        shape(reference, &declarations),
        Some(ValueShape::integer(8, 8))
    );
    assert_eq!(
        shape(array, &declarations),
        Some(ValueShape::integer(16, 8))
    );
    assert_eq!(
        project(
            array,
            &[StructuralPathSegment::FixedIndex(1)],
            &declarations
        ),
        Some((reference, 8))
    );
    assert!(
        project(
            array,
            &[StructuralPathSegment::FixedIndex(2)],
            &declarations
        )
        .is_none()
    );
    // Layout does not make a reference a constructible plain-owned aggregate.
    assert!(owned_aggregate_shape(reference, &declarations).is_none());
}

#[test]
fn fused_binding_borrow_keeps_pointer_abi_without_payload() {
    use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};
    let identity = StructuralTypeId::new(1).unwrap();
    let declarations = [StructuralTypeDeclaration {
        id: identity,
        identity: "Binding<Console>".into(),
        shape: StructuralTypeShape::FusedServiceBinding,
    }];
    assert_eq!(
        shape(identity, &declarations),
        Some(ValueShape::integer(0, 1))
    );
    assert!(owned_aggregate_shape(identity, &declarations).is_none());
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let parameter = StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: identity,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let borrowed = parameter_shape(&parameter, &declarations).unwrap();
        assert_eq!(borrowed, ValueShape::borrowed_reference(0, 1));
        for policy in [
            CallingPolicy::MicrosoftX64,
            CallingPolicy::SystemVAMD64,
            CallingPolicy::Aapcs64,
        ] {
            let plan = evaluate_call_plan(
                policy,
                &CallSignature {
                    parameters: vec![borrowed],
                    result: None,
                },
            )
            .unwrap();
            assert_eq!(plan.parameters[0].shape, borrowed);
            assert!(
                !plan.parameters[0].locations.is_empty(),
                "zero payload must not erase the borrowed pointer"
            );
        }
    }
}

/// Reconstruct parameter storage from its access, independently of body order.
pub(crate) fn parameter_shape(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    // Whole-root domain qualifications are signature preconditions the caller
    // or root installation discharges at invocation; they do not change the
    // parameter's storage shape. Projected qualifications still decline.
    if !parameter.projected_qualifications.is_empty() {
        return None;
    }
    if parameter.is_self
        && declarations.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
        })
    {
        return None;
    }
    if parameter.access == StructuralAccess::Owned
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
    {
        // Bounded bytes arrive as the same by-value `{length, bytes}`
        // aggregate their declaration lays out. No borrowed-pointer shape or
        // mutable-view authority follows from observing this owned payload.
        if abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
            parameter,
            declarations,
        ) {
            return shape(parameter.structural_type, declarations);
        }
        return primitive_array_shape(parameter.structural_type, declarations).or_else(|| {
            declarations
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .filter(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Sum { .. }
                            | StructuralTypeShape::Record { .. }
                            | StructuralTypeShape::Mixed { .. }
                    )
                })
                .and_then(|_| shape(parameter.structural_type, declarations))
        });
    }
    let referent = shape(parameter.structural_type, declarations)?;
    match (parameter.access, parameter.multiplicity) {
        // Receiver spelling does not change the owned payload's ABI geometry.
        (StructuralAccess::Owned, StructuralMultiplicity::Affine) => Some(referent),
        (
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine,
        ) if parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || declarations.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }
                    )
            }) =>
        {
            Some(ValueShape::borrowed_reference(
                referent.byte_size,
                referent.alignment,
            ))
        }
        _ => None,
    }
}

/// Borrow geometry for an exact plain record, including nested owned records.
/// This does not authorize constructing those nested fields.
pub(crate) fn plain_record_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    declarations
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .filter(|declaration| matches!(declaration.shape, StructuralTypeShape::Record { .. }))?;
    // Borrowing needs the complete referent footprint, including untouched byte
    // siblings. It does not need permission to construct or copy that payload.
    shape(structural_type, declarations)
}

/// Whole owned records, arrays, sums, and bounded bytes share payload geometry. This checks
/// carrier contents, not authority to construct, borrow or dispose the value.
/// Established inline byte fields travel with their capacity and length word;
/// their predicates remain in the exact type checked by the caller. A shared
/// view field travels as its two-word descriptor (backing pointer, extent):
/// a whole-value copy keeps the source loan's backing and extent, and Terminal
/// custody, not this geometry, bounds that loan. Exclusive and write-only views
/// cannot be duplicated by a copy and stay out. Geometry cannot establish
/// predicates or turn a borrowed descriptor into ownership, so a view root or
/// view array element stays a descriptor rather than an owned aggregate.
pub(crate) fn owned_aggregate_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    plain_aggregate(structural_type, declarations, &mut Vec::new()).then_some(())?;
    owned_storage_shape(structural_type, declarations)
}

/// Physical storage of a whole owned aggregate, independently of permission to
/// copy its contents. Existing erased fields occupy no bytes; nested loan
/// carriers retain their own geometry and independently checked custody.
/// Scalar places and bare loan descriptors do not become aggregate owners.
pub(crate) fn owned_storage_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    declarations
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .filter(|declaration| {
            matches!(
                declaration.shape,
                StructuralTypeShape::Record { .. }
                    | StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::Sum { .. }
                    | StructuralTypeShape::Mixed { .. }
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
            )
        })?;
    primitive_array_shape(structural_type, declarations)
        .or_else(|| shape(structural_type, declarations))
}

fn plain_aggregate(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    if active.contains(&structural_type) {
        return false;
    }
    let mut matching = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matching.next() else {
        return false;
    };
    if matching.next().is_some() {
        return false;
    }
    active.push(structural_type);
    let supported = match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            !matches!(scalar, ScalarType::Integer(integer) if integer.is_address())
                && scalar_shape(*scalar).is_some()
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            plain_aggregate(*element, declarations, active)
        }
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            ..
        }) => true,
        StructuralTypeShape::Sum { cases } => cases
            .iter()
            .flat_map(|case| case.fields.iter())
            .all(|field| plain_field(field, declarations, active)),
        StructuralTypeShape::Mixed { fields, cases } => fields
            .iter()
            .chain(cases.iter().flat_map(|case| case.fields.iter()))
            .all(|field| plain_field(field, declarations, active)),
        StructuralTypeShape::Record { fields } => fields
            .iter()
            .all(|field| plain_field(field, declarations, active)),
        _ => false,
    };
    active.pop();
    supported
}

/// One record, case or common field of a plain aggregate. Record, sum and
/// mixed carriers share this leaf classification, so a field kind admitted in
/// one carrier is admitted in all three.
fn plain_field(
    field: &terminal_psi::StructuralFieldDeclaration,
    declarations: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    !field.relevance.is_erased()
        && match field.field_type {
            StructuralFieldType::Structural(nested) => {
                // A reference leaf carries custody metadata, not transported
                // storage, so it shares the carrier's value layout as an
                // empty slot.
                declarations.iter().any(|declaration| {
                    declaration.id == nested
                        && matches!(declaration.shape, StructuralTypeShape::Reference { .. })
                }) || plain_aggregate(nested, declarations, active)
            }
            StructuralFieldType::Scalar(scalar) | StructuralFieldType::Atomic(scalar) => {
                scalar_shape(scalar).is_some()
            }
            StructuralFieldType::BoundedInteger(bounds) => {
                scalar_shape(ScalarType::Integer(bounds.integer_type())).is_some()
            }
            StructuralFieldType::RefinedScalar(ref refinement) => {
                scalar_shape(refinement.carrier()).is_some()
            }
            StructuralFieldType::IeeeFloat(format) => {
                scalar_shape(ScalarType::IeeeFloat(format)).is_some()
            }
            StructuralFieldType::ByteSequence(carrier)
            | StructuralFieldType::PredicateByteSequence { carrier, .. } => matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    | terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }
            ),
            StructuralFieldType::Erased { .. } => true,
        }
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

/// Reconstruct an initialized fixed-array loan, not an existing slice descriptor.
pub(crate) fn fixed_byte_array_view(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
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
    let (_, offset) = project(source.structural_type, window.backing_path, declarations)?;
    let root = shape(source.structural_type, declarations)?;
    if u64::from(offset).checked_add(window.backing_length)? > u64::from(root.byte_size) {
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
    (geometry.storage == terminal_semantics::CollectionStorage::Inline).then_some((
        geometry.offset,
        geometry.length,
        geometry.stride,
    ))
}

/// Native storage coordinates for an ordinary collection establishment.
/// A borrowed collection stores a pointer at `offset`; its declared backing
/// extent is independent of the bytes occupied by that pointer cell.
pub(crate) struct CollectionViewGeometry {
    pub offset: u32,
    pub length: u64,
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
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            ) | (
                StructuralAccess::Owned,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::Owned,
            )
        )
        || !(source.multiplicity == StructuralMultiplicity::Unrestricted
            || (source.multiplicity == StructuralMultiplicity::Affine
                && source.access == StructuralAccess::Owned
                && matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )))
        || !source.qualifications.is_empty()
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
    let view_element = match &declarations
        .iter()
        .find(|declaration| declaration.id == view_type)?
        .shape
    {
        StructuralTypeShape::ElementView {
            element: view_element,
        } => Some(*view_element),
        shape if shape.is_borrowed_byte_view() => None,
        _ => return None,
    };
    let (tip, offset) = project(source.structural_type, field_path, declarations)?;
    let element = view_element.or_else(|| {
        declarations.iter().find_map(|declaration| {
            matches!(declaration.shape,
            StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
            if Ok(integer) == semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned, 8))
            .then_some(declaration.id)
            .filter(|element| {
                terminal_semantics::collection_backing(declarations.iter(), tip, *element).is_some()
            })
        })
    })?;
    let geometry = terminal_semantics::element_view_region_geometry(
        declarations.iter(),
        tip,
        element,
        region,
        argument.access,
    )?;
    if region == terminal_psi::ElementViewRegion::WholeCollection
        && !typed_collection_layout_matches(tip, element, argument.access, declarations)
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
            let storage = if matches!(
                declarations.iter().find(|row| row.id == tip)?.shape,
                StructuralTypeShape::ByteSequence(_)
            ) {
                terminal_semantics::CollectionStorage::Borrowed
            } else {
                terminal_semantics::CollectionStorage::Inline
            };
            (
                terminal_semantics::CollectionBacking {
                    length: byte_extent,
                    storage,
                },
                Some(layout),
            )
        }
    };
    if backing.storage == terminal_semantics::CollectionStorage::Borrowed
        && (byte_window.is_some() || (view_element.is_some() && representation.is_none()))
    {
        return None;
    }
    let length = backing.length;
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
    let (window_offset, element_count) = match byte_window {
        Some((start, end)) if start <= end && end <= length => {
            let byte_offset = start.checked_mul(u64::from(element_stride))?;
            (u32::try_from(byte_offset).ok()?, end - start)
        }
        Some(_) => return None,
        None => (0, length),
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
        terminal_semantics::CollectionStorage::Borrowed => 8,
    };
    let end = u64::from(offset).checked_add(storage_size)?;
    if end > u64::from(shape(source.structural_type, declarations)?.byte_size) {
        return None;
    }
    Some(CollectionViewGeometry {
        offset,
        length: element_count,
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
pub(crate) fn bounded_byte_view(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
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
        return bounded_byte_field_borrow(source, argument, view_type, declarations, false);
    }
    let expected = terminal_psi::StructuralParameterDeclaration {
        place: argument.place,
        position: 0,
        is_self: false,
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
    if argument.path.is_empty() {
        return Some((0, capacity));
    }
    let (offset, field_capacity) =
        bounded_byte_field_borrow(source, argument, view_type, declarations, false)?;
    (field_capacity == capacity).then_some((offset, capacity))
}

/// A stored shared view field lent whole to a shared view formal: the
/// argument points at the field's own two-word descriptor, which is the
/// formal's referent. Returns that descriptor's offset in the root. No
/// descriptor is synthesized and no backing byte is addressed.
pub(crate) fn stored_view_field_argument(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<u32> {
    use terminal_psi::StructuralAccess;
    let Some((StructuralPathSegment::Field(name), prefix)) = argument.path.split_last() else {
        return None;
    };
    if source.place != argument.place
        || argument.access != StructuralAccess::SharedBorrow
        || !matches!(
            source.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        )
        || !declarations.iter().any(|declaration| {
            declaration.id == view_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                        }
                    )
                )
        })
    {
        return None;
    }
    let StructuralTypeShape::Record { fields } = &terminal_semantics::runtime_structural_path_tip(
        declarations.iter(),
        source.structural_type,
        prefix,
    )?
    .shape
    else {
        return None;
    };
    let field = fields
        .iter()
        .find(|field| field.identity == *name && !field.relevance.is_erased())?;
    byte_view_field_descriptor(source.structural_type, prefix, field.id, declarations)
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
    let Some((StructuralPathSegment::Field(name), prefix)) = argument.path.split_last() else {
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
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
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
    let (carrier, carrier_offset) = if prefix.is_empty() {
        (source.structural_type, 0)
    } else {
        project(source.structural_type, prefix, declarations)?
    };
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let mut local_offset = 0_u32;
    for candidate in fields.iter().filter(|field| {
        !field.relevance.is_erased()
            && !matches!(field.field_type, StructuralFieldType::Erased { .. })
    }) {
        let layout = field_shape(&candidate.field_type, declarations, &mut Vec::new())?;
        local_offset = align(local_offset, layout.alignment)?;
        if candidate.identity == *name {
            let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) =
                candidate.field_type.byte_sequence_carrier()
            else {
                return None;
            };
            if owned
                && !declarations.iter().any(|declaration| {
                    declaration.id == view_type
                        && declaration.shape
                            == StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                            )
                })
            {
                return None;
            }
            let field_offset = carrier_offset.checked_add(local_offset)?;
            return (u64::from(field_offset)
                .checked_add(8)?
                .checked_add(capacity)?
                <= u64::from(shape(source.structural_type, declarations)?.byte_size))
            .then_some((field_offset, capacity));
        }
        local_offset = local_offset.checked_add(layout.byte_size)?;
    }
    None
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
    let declaration = declarations
        .iter()
        .find(|declaration| declaration.id == root)?;
    let result = match &declaration.shape {
        // Borrowing preserves a pointer to the established occurrence even
        // though the service marker itself contributes no payload bytes.
        StructuralTypeShape::FusedServiceBinding => Some(ValueShape::integer(0, 1)),
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
            let payloads = cases
                .iter()
                .map(|case| {
                    case.fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .map(|field| field_shape(&field.field_type, declarations, active))
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
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
            let payloads = cases
                .iter()
                .map(|case| {
                    case.fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .map(|field| field_shape(&field.field_type, declarations, active))
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
            Some(
                calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
                    .ok()?
                    .shape,
            )
        }
        // A reference owns permission, not its referent's bytes. Its payload
        // transports one address; custody remains independently reconstructed.
        StructuralTypeShape::Reference { .. } => Some(ValueShape::integer(8, 8)),
        StructuralTypeShape::Record { fields } => {
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
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == current)?;
        let (next, displacement) = match segment {
            Segment::Case(identity) => {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return None;
                };
                let case_position = cases.iter().position(|case| case.id == *identity)?;
                let Segment::Field(field) = segments.next()? else {
                    return None;
                };
                let (field_position, field) = cases[case_position]
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, candidate)| candidate.id == *field)?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
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
                let (StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. }) = &declaration.shape
                else {
                    return None;
                };
                let (field_position, field) = fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.id == *identity)?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
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
    let declaration = declarations
        .iter()
        .find(|declaration| declaration.id == root)?;
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

pub(crate) fn project(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32)> {
    project_with_leaf_shape(
        root,
        path,
        declarations,
        StructuralFieldType::canonical_leaf_shape,
    )
}

fn project_with_leaf_shape(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
    leaf_shape: fn(&StructuralFieldType) -> Option<StructuralTypeShape>,
) -> Option<(StructuralTypeId, u32)> {
    project_inner(
        root,
        path.iter().map(|segment| match segment {
            StructuralPathSegment::Field(name) => Projection::FieldName(name),
            StructuralPathSegment::Case(case) => Projection::Case(*case),
            StructuralPathSegment::FixedIndex(position) => Projection::FixedIndex(*position),
            _ => Projection::Unsupported,
        }),
        declarations,
        leaf_shape,
    )
}

enum Projection<'a> {
    Case(semantic_vocabulary::StructuralCaseId),
    FieldName(&'a str),
    FixedIndex(u64),
    Unsupported,
}

fn project_inner<'a>(
    root: StructuralTypeId,
    path: impl Iterator<Item = Projection<'a>>,
    declarations: &[StructuralTypeDeclaration],
    leaf_shape: fn(&StructuralFieldType) -> Option<StructuralTypeShape>,
) -> Option<(StructuralTypeId, u32)> {
    let mut carrier = root;
    let mut offset = 0_u32;
    shape(root, declarations)?;
    let mut path = path;
    while let Some(segment) = path.next() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == carrier)?;
        match (segment, &declaration.shape) {
            (
                Projection::Case(case),
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
            ) => {
                let case_position = cases.iter().position(|candidate| candidate.id == case)?;
                let Projection::FieldName(identity) = path.next()? else {
                    return None;
                };
                let (field_position, field) = cases[case_position]
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.identity == identity)?;
                let StructuralFieldType::Structural(selected) = field.field_type else {
                    return None;
                };
                let layout = case_layout(carrier, declarations)?;
                offset = offset.checked_add(
                    layout
                        .cases
                        .get(case_position)?
                        .fields
                        .get(field_position)?
                        .byte_offset,
                )?;
                carrier = selected;
            }
            (
                Projection::FixedIndex(position),
                StructuralTypeShape::FixedArray { element, length },
            ) if position < *length => {
                let element_shape = shape(*element, declarations)?;
                let stride = align(element_shape.byte_size, element_shape.alignment)?;
                offset = offset
                    .checked_add(u32::try_from(u64::from(stride).checked_mul(position)?).ok()?)?;
                carrier = *element;
            }
            (Projection::FixedIndex(position), StructuralTypeShape::ElementView { element }) => {
                // A view's referent storage is element-contiguous; its
                // stored extent, not a literal length, bounds the index.
                let element_shape = shape(*element, declarations)?;
                let stride = align(element_shape.byte_size, element_shape.alignment)?;
                offset = offset
                    .checked_add(u32::try_from(u64::from(stride).checked_mul(position)?).ok()?)?;
                carrier = *element;
            }
            (Projection::FieldName(name), StructuralTypeShape::Mixed { fields, .. }) => {
                let (position, field) = fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.identity == name)?;
                let nested = match &field.field_type {
                    StructuralFieldType::Structural(nested) => *nested,
                    leaf => {
                        let shape = leaf_shape(leaf)?;
                        declarations
                            .iter()
                            .find(|declaration| declaration.shape == shape)?
                            .id
                    }
                };
                let field_offset = case_layout(carrier, declarations)?
                    .common_fields
                    .get(position)?
                    .byte_offset;
                carrier = nested;
                offset = offset.checked_add(field_offset)?;
            }
            (Projection::FieldName(name), StructuralTypeShape::Record { fields }) => {
                let mut field_offset = 0;
                let mut found = None;
                for field in fields.iter().filter(|field| {
                    !field.relevance.is_erased()
                        && !matches!(field.field_type, StructuralFieldType::Erased { .. })
                }) {
                    let layout = field_shape(&field.field_type, declarations, &mut Vec::new())?;
                    field_offset = align(field_offset, layout.alignment)?;
                    if field.identity == name {
                        let nested = match &field.field_type {
                            StructuralFieldType::Structural(nested) => *nested,
                            leaf => {
                                let shape = leaf_shape(leaf)?;
                                declarations
                                    .iter()
                                    .find(|declaration| declaration.shape == shape)
                                    .map(|declaration| declaration.id)?
                            }
                        };
                        found = Some((nested, field_offset));
                        break;
                    }
                    field_offset = field_offset.checked_add(layout.byte_size)?;
                }
                let (nested, field_offset) = found?;
                carrier = nested;
                offset = offset.checked_add(field_offset)?;
            }
            _ => return None,
        }
    }
    Some((carrier, offset))
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
    runtime_projection_with_leaf_shape(
        root,
        path,
        declarations,
        StructuralFieldType::canonical_leaf_shape,
    )
}

/// Copy snapshots expose the declaration's byte payload and live length, not
/// permission to use a byte field as an ordinary borrowed call operand.
pub(crate) fn runtime_leaf_copy_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
    runtime_projection_with_leaf_shape(
        root,
        path,
        declarations,
        StructuralFieldType::leaf_copy_shape,
    )
}

fn runtime_projection_with_leaf_shape(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
    leaf_shape: fn(&StructuralFieldType) -> Option<StructuralTypeShape>,
) -> Option<(StructuralTypeId, u32, Vec<RuntimeElement>)> {
    let mut container = root;
    let mut byte_offset = 0u32;
    let mut elements = Vec::new();
    let mut segment_start = 0usize;
    for (position, segment) in path.iter().enumerate() {
        let StructuralPathSegment::RuntimeIndex { index, obligation } = segment else {
            continue;
        };
        if position > segment_start {
            let (selected, run_offset) = project_with_leaf_shape(
                container,
                &path[segment_start..position],
                declarations,
                leaf_shape,
            )?;
            container = selected;
            byte_offset = byte_offset.checked_add(run_offset)?;
        }
        let (element, extent) = match &declarations
            .iter()
            .find(|declaration| declaration.id == container)?
            .shape
        {
            StructuralTypeShape::FixedArray { element, length } => (*element, Some(*length)),
            // A view's extent is its own stored runtime length — the bound
            // re-proves against the dominating length observation the
            // obligation names, never against this layout.
            StructuralTypeShape::ElementView { element } => (*element, None),
            _ => return None,
        };
        let element_shape = shape(element, declarations)?;
        let stride = align(element_shape.byte_size, element_shape.alignment)?;
        elements.push(RuntimeElement {
            index: *index,
            obligation: *obligation,
            stride,
            extent,
        });
        container = element;
        segment_start = position + 1;
    }
    let (endpoint, tail_offset) =
        project_with_leaf_shape(container, &path[segment_start..], declarations, leaf_shape)?;
    Some((endpoint, byte_offset.checked_add(tail_offset)?, elements))
}

/// The field-store geometry: the static offset of `field` beneath the
/// carrier `path`, its width, and each runtime element of the carrier in
/// path order. The carrier composes record fields and literal or runtime
/// elements in any order (`is_structural_scalar_store_path`).
pub(crate) fn store(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8, Vec<RuntimeElement>)> {
    // The owner retains the reference carrier and explicit crossing. Layout
    // continues inside its mutable record referent; this supplies geometry,
    // never permission or evidence that the carrier is currently live.
    let (root, path) = if let Some((StructuralPathSegment::Referent, suffix)) = path.split_first() {
        let StructuralTypeShape::Reference {
            referent,
            access: terminal_psi::StructuralAccess::MutableBorrow,
        } = declarations
            .iter()
            .find(|declaration| declaration.id == root)?
            .shape
        else {
            return None;
        };
        if !declarations.iter().any(|declaration| {
            declaration.id == referent
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        }) {
            return None;
        }
        (referent, suffix)
    } else {
        (root, path)
    };
    if !terminal_psi::is_structural_scalar_store_path(path) {
        return None;
    }
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
) -> Option<(u32, u8)> {
    if terminal_semantics::element_view_leaf_scalar(declarations, element, path) != Some(scalar) {
        return None;
    }
    if let Some((StructuralPathSegment::Field(identity), prefix)) = path.split_last() {
        let (parent, _) = project(element, prefix, declarations)?;
        let StructuralTypeShape::Record { fields } = &declarations
            .iter()
            .find(|declaration| declaration.id == parent)?
            .shape
        else {
            return None;
        };
        let field = fields.iter().find(|field| field.identity == *identity)?;
        if field.field_type.scalar_type() == Some(scalar) {
            return scalar_field_geometry(element, prefix, field.id, scalar, declarations, false)
                .map(|(offset, bytes, _)| (offset, bytes));
        }
    }
    primitive_geometry(element, path, scalar, declarations)
        .map(|(offset, bytes, _)| (offset, bytes))
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
) -> Option<(u32, u8, Vec<RuntimeElement>)> {
    let (carrier, carrier_offset, elements) = runtime_projection(root, path, declarations)?;
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let mut offset = 0;
    for candidate in fields.iter().filter(|field| {
        !field.relevance.is_erased()
            && !matches!(field.field_type, StructuralFieldType::Erased { .. })
    }) {
        let layout = field_shape(&candidate.field_type, declarations, &mut Vec::new())?;
        offset = align(offset, layout.alignment)?;
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
                    if declarations
                        .iter()
                        .find(|declaration| declaration.id == child)
                        .is_some_and(|declaration| {
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
            let bytes = u8::try_from(scalar_shape(scalar)?.byte_size).ok()?;
            // A view root's descriptor is not the write's frame: the
            // referent's extent bounds the store, so the descriptor-size
            // ceiling applies only to storage the path stays inside.
            let descriptor_frame = matches!(
                declarations
                    .iter()
                    .find(|declaration| declaration.id == root)
                    .map(|declaration| &declaration.shape),
                Some(
                    StructuralTypeShape::ElementView { .. }
                        | StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                )
            );
            if !descriptor_frame
                && u64::from(offset)
                    .checked_add(runtime_reach(&elements)?)?
                    .checked_add(u64::from(bytes))?
                    > u64::from(shape(root, declarations)?.byte_size)
            {
                return None;
            }
            return Some((offset, bytes, elements));
        }
        offset = offset.checked_add(layout.byte_size)?;
    }
    None
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
) -> Option<(u32, u8)> {
    if !matches!(scalar, ScalarType::Boolean | ScalarType::Integer(_))
        || !path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        return None;
    }
    // An element view is a descriptor, not the observed storage: its live
    // extent — not a root record footprint — bounds the read. A fixed array
    // roots its literal element's field read as a record roots its member's:
    // every literal index selects its declared element, and the whole
    // array footprint bounds the read.
    let root_shape = &declarations
        .iter()
        .find(|declaration| declaration.id == structural_type)?
        .shape;
    let root = match root_shape {
        StructuralTypeShape::ElementView { .. } => None,
        StructuralTypeShape::FixedArray { .. } => Some(shape(structural_type, declarations)?),
        _ => Some(plain_record_shape(structural_type, declarations)?),
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
) -> Option<(u32, u8)> {
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
) -> Option<(u32, u8)> {
    let StructuralTypeShape::Mixed { fields, .. } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
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
    let bytes = u8::try_from(scalar_shape(scalar)?.byte_size).ok()?;
    (placed.shape.byte_size == u32::from(bytes)).then_some((placed.byte_offset, bytes))
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
) -> Option<(u32, u8)> {
    if !matches!(scalar, ScalarType::Boolean | ScalarType::Integer(_))
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
    {
        return None;
    }
    let root = shape(structural_type, declarations)?;
    let (sum, sum_offset) = project(structural_type, path, declarations)?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declarations
            .iter()
            .find(|declaration| declaration.id == sum)?
            .shape
    else {
        return None;
    };
    let case_position = cases.iter().position(|candidate| candidate.id == case)?;
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
    let bytes = u8::try_from(scalar_shape(scalar)?.byte_size).ok()?;
    let offset = sum_offset.checked_add(placed.byte_offset)?;
    (placed.shape.byte_size == u32::from(bytes)
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
}

pub(crate) fn projected_byte_field(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ProjectedByteField> {
    let count = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .ok()?,
    );
    let (length_offset, width, elements) =
        scalar_field_geometry(root, path, field, count, declarations, true)?;
    if width != 8 {
        return None;
    }
    let (carrier, _, _) = runtime_projection(root, path, declarations)?;
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let mut matching = fields
        .iter()
        .filter(|candidate| candidate.id == field && !candidate.relevance.is_erased());
    let selected = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let capacity = match selected.field_type.byte_sequence_carrier() {
        Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) => Some(capacity),
        _ => None,
    };
    // The scalar geometry owner admitted only length-bearing byte or element
    // descriptors; an inline owner alone places its length before its bytes.
    let descriptor_offset = if capacity.is_some() {
        None
    } else {
        Some(length_offset.checked_sub(8)?)
    };
    Some(ProjectedByteField {
        length_offset,
        descriptor_offset,
        capacity,
        elements,
    })
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
) -> Option<(u32, u8)> {
    if !terminal_psi::is_bounded_structural_scalar_store_path(path) {
        return None;
    }
    plain_record_shape(structural_type, declarations)?;
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
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
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

/// A shared view field stores a two-word descriptor in its record: the
/// backing pointer, then the extent. Return the descriptor's offset from the
/// root. The backing itself lies outside the root; its custody is the view's.
pub(crate) fn byte_view_field_descriptor(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<u32> {
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .ok()?,
    );
    let (length_word, _) = byte_field_length(structural_type, path, field, scalar, declarations)?;
    let (carrier, _) = project(structural_type, path, declarations)?;
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let mut matching = fields.iter().filter(|candidate| candidate.id == field);
    let selected = matching.next()?;
    (matching.next().is_none()
        && (selected.field_type.byte_sequence_carrier()
            == Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            })
            || matches!(
                selected.field_type,
                StructuralFieldType::Structural(child)
                    if declarations.iter().any(|declaration| {
                        declaration.id == child
                            && matches!(
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
                    })
            )))
    .then(|| length_word.checked_sub(8))
    .flatten()
}

#[cfg(test)]
mod tests;

#[test]
fn affine_array_view_geometry_requires_a_borrow_not_an_owned_transfer() {
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration,
    };
    let element = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let view = StructuralTypeId::new(3).unwrap();
    let declarations = [
        StructuralTypeDeclaration {
            id: element,
            identity: "flag".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "flags".into(),
            shape: StructuralTypeShape::FixedArray { element, length: 7 },
        },
        StructuralTypeDeclaration {
            id: view,
            identity: "view".into(),
            shape: StructuralTypeShape::ElementView { element },
        },
    ];
    let source = StructuralParameterDeclaration {
        place: semantic_vocabulary::PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type: array,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut argument = StructuralArgument {
        place: source.place,
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        argument.access = access;
        assert_eq!(
            fixed_element_array_view(&source, &argument, view, &declarations),
            Some((0, 7, 1))
        );
    }
    argument.access = StructuralAccess::Owned;
    assert!(fixed_element_array_view(&source, &argument, view, &declarations).is_none());
    argument.access = StructuralAccess::SharedBorrow;
    let mut linear = source.clone();
    linear.multiplicity = StructuralMultiplicity::Linear;
    assert!(fixed_element_array_view(&linear, &argument, view, &declarations).is_none());
    argument.place = semantic_vocabulary::PlaceId::new(2).unwrap();
    assert!(fixed_element_array_view(&source, &argument, view, &declarations).is_none());
}

#[test]
fn bounded_byte_collection_geometry_separates_capacity_from_pointer_storage() {
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess as Access, StructuralArgument,
        StructuralMultiplicity, StructuralParameterDeclaration,
    };
    let element = StructuralTypeId::new(901).unwrap();
    let bounded = StructuralTypeId::new(902).unwrap();
    let view = StructuralTypeId::new(903).unwrap();
    let typed_view = StructuralTypeId::new(904).unwrap();
    let source = StructuralParameterDeclaration {
        place: semantic_vocabulary::PlaceId::new(901).unwrap(),
        position: 0,
        is_self: false,
        structural_type: bounded,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: Access::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let argument = StructuralArgument {
        place: source.place,
        path: Vec::new(),
        access: Access::MutableBorrow,
    };
    for capacity in [0, 3, 17] {
        let mut declarations = vec![
            StructuralTypeDeclaration {
                id: element,
                identity: "octet".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        8,
                    )
                    .unwrap(),
                )),
            },
            StructuralTypeDeclaration {
                id: bounded,
                identity: "bounded".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                    capacity,
                    access: None,
                }),
            },
            StructuralTypeDeclaration {
                id: view,
                identity: "bytes".into(),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                }),
            },
            StructuralTypeDeclaration {
                id: typed_view,
                identity: "elements".into(),
                shape: StructuralTypeShape::ElementView { element },
            },
        ];
        let geometry = collection_view_geometry(&source, &argument, view, &declarations).unwrap();
        assert_eq!(geometry.offset, 0);
        assert_eq!(geometry.length, capacity);
        assert_eq!(geometry.stride, 1);
        assert_eq!(
            geometry.storage,
            terminal_semantics::CollectionStorage::Borrowed
        );
        assert_eq!(shape(bounded, &declarations).unwrap().byte_size, 8);
        assert!(
            fixed_element_array_view(&source, &argument, view, &declarations).is_none(),
            "a pointer carrier is not inline array storage"
        );
        assert!(
            collection_view_geometry(&source, &argument, typed_view, &declarations).is_none(),
            "byte backing cannot be reinterpreted as an element descriptor"
        );
        let mut shared = source.clone();
        shared.access = Access::SharedBorrow;
        assert!(collection_view_geometry(&shared, &argument, view, &declarations).is_none());
        let mut attenuated = argument.clone();
        attenuated.access = Access::SharedBorrow;
        assert!(collection_view_geometry(&shared, &attenuated, view, &declarations).is_some());
        declarations[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        assert!(collection_view_geometry(&source, &argument, view, &declarations).is_none());
    }
}

/// A typed representation loan keeps both declarations. Portable validity
/// grants no target layout authority: replay each realized leaf and padded
/// size independently before transporting the original backing pointer.
pub(crate) fn typed_collection_layout_matches(
    collection: StructuralTypeId,
    target: StructuralTypeId,
    access: terminal_psi::StructuralAccess,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Some(StructuralTypeShape::FixedArray {
        element: source, ..
    }) = declarations
        .iter()
        .find(|row| row.id == collection)
        .map(|row| &row.shape)
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
