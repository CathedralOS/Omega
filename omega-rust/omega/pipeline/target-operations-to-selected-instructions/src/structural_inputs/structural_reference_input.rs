//! Input-only reconstruction of structural storage and borrowed-pointer geometry.
//! Whole owned aggregates and borrowed referents share recursive payload layout;
//! their callers independently check access, ownership and exact ABI placement.
use calling_conventions::{
    IndirectPointerLocation, ValueClass, ValueLocation, ValuePlacement, ValueShape,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ScalarType, StructuralFieldId, StructuralTypeId,
};
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

fn align(value: u32, alignment: u16) -> Option<u32> {
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
            Some(ValueShape::integer(bytes, bytes))
        }
        _ => None,
    }
}

/// Reconstruct a primitive write without inventing a carrier record or field.
pub(crate) fn primitive_store(
    destination: &terminal_psi::StructuralParameterDeclaration,
    path: &[CanonicalStructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8)> {
    if destination.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || (path.is_empty()
            && destination.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted)
        || !matches!(
            destination.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || matches!(scalar, ScalarType::Integer(integer) if integer.is_address())
    {
        return None;
    }
    primitive_geometry(destination.structural_type, path, scalar, declarations)
}

/// Reconstruct the selected primitive and its footprint within the original root.
pub(crate) fn primitive_geometry(
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8)> {
    let (leaf, offset) = project_inner(
        root,
        path.iter().map(|segment| match segment {
            CanonicalStructuralPathSegment::Field(field) => Projection::FieldId(*field),
            CanonicalStructuralPathSegment::FixedIndex(position) => {
                Projection::FixedIndex(*position)
            }
            _ => Projection::Unsupported,
        }),
        declarations,
    )?;
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == leaf);
    let declaration = matches.next()?;
    if matches.next().is_some() || declaration.shape != StructuralTypeShape::PrimitiveScalar(scalar)
    {
        return None;
    }
    let bytes = u8::try_from(scalar_shape(scalar)?.byte_size).ok()?;
    (offset.checked_add(u32::from(bytes))? <= u32::from(shape(root, declarations)?.byte_size))
        .then_some((offset, bytes))
}

pub(crate) fn shape(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    shape_inner(root, declarations, &mut Vec::new())
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
        && !parameter.is_self
    {
        return primitive_array_shape(parameter.structural_type, declarations).or_else(|| {
            declarations
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .filter(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Sum { .. } | StructuralTypeShape::Record { .. }
                    )
                })
                .and_then(|_| shape(parameter.structural_type, declarations))
        });
    }
    let referent = shape(parameter.structural_type, declarations)?;
    match (parameter.access, parameter.multiplicity) {
        (StructuralAccess::Owned, StructuralMultiplicity::Affine) if !parameter.is_self => {
            Some(referent)
        }
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

/// Whole owned records, arrays, and sums share payload geometry. This checks
/// carrier contents, not authority to construct, borrow or dispose the value.
pub(crate) fn owned_aggregate_shape(
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
            )
        })?;
    plain_aggregate(structural_type, declarations, &mut Vec::new()).then_some(())?;
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
        StructuralTypeShape::Sum { cases } => cases.iter().all(|case| {
            case.fields.iter().all(|field| {
                !field.relevance.is_erased()
                    && field
                        .field_type
                        .scalar_type()
                        .and_then(scalar_shape)
                        .is_some()
            })
        }),
        StructuralTypeShape::Record { fields } => fields.iter().all(|field| {
            !field.relevance.is_erased()
                && match field.field_type {
                    StructuralFieldType::Structural(nested) => {
                        plain_aggregate(nested, declarations, active)
                    }
                    StructuralFieldType::Scalar(scalar) => scalar_shape(scalar).is_some(),
                    StructuralFieldType::BoundedInteger(bounds) => {
                        scalar_shape(ScalarType::Integer(bounds.integer_type())).is_some()
                    }
                    StructuralFieldType::IeeeFloat(format) => {
                        scalar_shape(ScalarType::IeeeFloat(format)).is_some()
                    }
                    StructuralFieldType::Erased { .. } => true,
                    _ => false,
                }
        }),
        _ => false,
    };
    active.pop();
    supported
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
                let size = u16::try_from(
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

/// Reconstruct a borrowed view presented from a bounded inline byte field.
/// The field's live length word and capacity bytes stay in the caller's record
/// storage; the descriptor staged for the call reads that length in place.
/// This returns metadata geometry only — it grants neither byte access nor
/// authority to replace the field.
pub(crate) fn bounded_byte_field_view(
    source: &terminal_psi::StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    view_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let Some((StructuralPathSegment::Field(name), prefix)) = argument.path.split_last() else {
        return None;
    };
    if source.place != argument.place
        || !matches!(
            (source.access, argument.access),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow,
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            )
        )
        || source.multiplicity != StructuralMultiplicity::Unrestricted
        || !source.qualifications.is_empty()
        || !source.projected_qualifications.is_empty()
        || !declarations.iter().any(|declaration| {
            declaration.id == view_type
                && declaration.shape
                    == StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    )
        })
    {
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
            let StructuralFieldType::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
            ) = candidate.field_type
            else {
                return None;
            };
            let field_offset = carrier_offset.checked_add(local_offset)?;
            return (u64::from(field_offset)
                .checked_add(8)?
                .checked_add(capacity)?
                <= u64::from(shape(source.structural_type, declarations)?.byte_size))
            .then_some((field_offset, capacity));
        }
        local_offset = local_offset.checked_add(u32::from(layout.byte_size))?;
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
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_shape(*scalar),
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView) => {
            Some(ValueShape::integer(16, 8))
        }
        StructuralTypeShape::Sum { cases } => {
            let payloads = cases
                .iter()
                .map(|case| {
                    case.fields
                        .iter()
                        .map(|field| {
                            if field.relevance.is_erased() {
                                return None;
                            }
                            let ScalarType::Integer(integer) = field.field_type.scalar_type()?
                            else {
                                return None;
                            };
                            scalar_shape(ScalarType::Integer(integer))
                        })
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
        // A reference carrier transports no referent storage: custody is
        // compile-time metadata, so its shape is an empty aggregate slot
        // rather than a pointer-sized payload.
        StructuralTypeShape::Reference { .. } => Some(ValueShape::integer(0, 1)),
        StructuralTypeShape::Record { fields } => {
            let mut bytes = 0;
            let mut alignment = 1;
            for field in fields.iter().filter(|field| {
                !field.relevance.is_erased()
                    && !matches!(field.field_type, StructuralFieldType::Erased { .. })
            }) {
                let field_shape = field_shape(&field.field_type, declarations, active)?;
                alignment = alignment.max(field_shape.alignment);
                bytes = align(bytes, field_shape.alignment)?
                    .checked_add(u32::from(field_shape.byte_size))?;
            }
            Some(ValueShape::integer(
                u16::try_from(align(bytes, alignment)?).ok()?,
                alignment,
            ))
        }
        StructuralTypeShape::FixedArray { element, length } if *length > 0 => {
            let element = shape_inner(*element, declarations, active)?;
            let stride = align(u32::from(element.byte_size), element.alignment)?;
            Some(ValueShape::integer(
                u16::try_from(u64::from(stride).checked_mul(*length)?).ok()?,
                element.alignment,
            ))
        }
        _ => None,
    };
    active.pop();
    result
}

fn field_shape(
    field: &StructuralFieldType,
    declarations: &[StructuralTypeDeclaration],
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    match field {
        StructuralFieldType::Scalar(scalar) => scalar_shape(*scalar),
        // Geometry follows the carrier, not the restriction's endpoints.
        // Store admission below still requires separate restricted-field evidence.
        StructuralFieldType::BoundedInteger(bounds) => {
            scalar_shape(ScalarType::Integer(bounds.integer_type()))
        }
        StructuralFieldType::IeeeFloat(format) => scalar_shape(ScalarType::IeeeFloat(*format)),
        StructuralFieldType::ByteSequence(carrier) => {
            // Match target layout: an inline bounded buffer retains capacity
            // bytes and a length word; a borrowed view retains two words. This
            // supplies sibling offsets only, not byte access or replacement.
            let bytes = match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    capacity.checked_add(8)?
                }
                terminal_psi::ByteSequenceCarrier::BorrowedView => 16,
            };
            Some(ValueShape::integer(u16::try_from(bytes).ok()?, 8))
        }
        StructuralFieldType::Structural(nested) => shape_inner(*nested, declarations, active),
        _ => None,
    }
}

pub(crate) fn project(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32)> {
    project_inner(
        root,
        path.iter().map(|segment| match segment {
            StructuralPathSegment::Field(name) => Projection::FieldName(name),
            StructuralPathSegment::FixedIndex(position) => Projection::FixedIndex(*position),
            _ => Projection::Unsupported,
        }),
        declarations,
    )
}

enum Projection<'a> {
    FieldName(&'a str),
    FieldId(StructuralFieldId),
    FixedIndex(u64),
    Unsupported,
}

fn project_inner<'a>(
    root: StructuralTypeId,
    path: impl Iterator<Item = Projection<'a>>,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, u32)> {
    let mut carrier = root;
    let mut offset = 0_u32;
    shape(root, declarations)?;
    for segment in path {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == carrier)?;
        match (segment, &declaration.shape) {
            (
                Projection::FixedIndex(position),
                StructuralTypeShape::FixedArray { element, length },
            ) if position < *length => {
                let element_shape = shape(*element, declarations)?;
                let stride = align(u32::from(element_shape.byte_size), element_shape.alignment)?;
                offset = offset
                    .checked_add(u32::try_from(u64::from(stride).checked_mul(position)?).ok()?)?;
                carrier = *element;
            }
            (
                projection @ (Projection::FieldName(_) | Projection::FieldId(_)),
                StructuralTypeShape::Record { fields },
            ) => {
                let mut field_offset = 0;
                let mut found = None;
                for field in fields.iter().filter(|field| {
                    !field.relevance.is_erased()
                        && !matches!(field.field_type, StructuralFieldType::Erased { .. })
                }) {
                    let layout = field_shape(&field.field_type, declarations, &mut Vec::new())?;
                    field_offset = align(field_offset, layout.alignment)?;
                    if match projection {
                        Projection::FieldName(name) => field.identity == name,
                        Projection::FieldId(identity) => field.id == identity,
                        _ => false,
                    } {
                        let nested = match &field.field_type {
                            StructuralFieldType::Structural(nested) => *nested,
                            leaf => {
                                let shape = leaf.canonical_leaf_shape()?;
                                declarations
                                    .iter()
                                    .find(|declaration| declaration.shape == shape)
                                    .map(|declaration| declaration.id)?
                            }
                        };
                        found = Some((nested, field_offset));
                        break;
                    }
                    field_offset = field_offset.checked_add(u32::from(layout.byte_size))?;
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

pub(crate) fn store(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8)> {
    if !terminal_psi::is_bounded_structural_scalar_store_path(path) {
        return None;
    }
    // Current-IR validation binds every bounded write to its accepted range
    // proposition. This helper checks physical geometry, not proof authority.
    scalar_field_geometry(root, path, field, scalar, declarations, false)
}

/// Reads and proven writes share carrier geometry. Neither this layout nor a
/// constructor/entry range proof supplies the separate authority for a write.
fn scalar_field_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
    observes_byte_length: bool,
) -> Option<(u32, u8)> {
    let (carrier, carrier_offset) = project(root, path, declarations)?;
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
            let matches_type = match candidate.field_type {
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => {
                    observes_byte_length
                        && matches!(scalar, ScalarType::Integer(integer)
                        if Ok(integer) == semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned, 64))
                }
                StructuralFieldType::Scalar(actual) => !observes_byte_length && actual == scalar,
                StructuralFieldType::IeeeFloat(format) => {
                    !observes_byte_length && ScalarType::IeeeFloat(format) == scalar
                }
                StructuralFieldType::BoundedInteger(bounds) => {
                    !observes_byte_length && ScalarType::Integer(bounds.integer_type()) == scalar
                }
                _ => false,
            };
            if !matches_type {
                return None;
            }
            let offset = carrier_offset.checked_add(offset)?;
            let bytes = u8::try_from(scalar_shape(scalar)?.byte_size).ok()?;
            if offset.checked_add(u32::from(bytes))?
                > u32::from(shape(root, declarations)?.byte_size)
            {
                return None;
            }
            return Some((offset, bytes));
        }
        offset = offset.checked_add(u32::from(layout.byte_size))?;
    }
    None
}

/// Exact bounded carrier geometry: record fields, optionally followed by one
/// literal fixed-array index — the same grammar the store projection shares.
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
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
    {
        return None;
    }
    plain_record_shape(structural_type, declarations)?;
    scalar_field_geometry(structural_type, path, field, scalar, declarations, false)
}

/// Bounded inline byte storage starts with an aligned u64 live length, followed
/// by capacity bytes. The enclosing record rounds the capacity+8 footprint for
/// its next field; a borrowed view's pointer/length descriptor is a different
/// carrier. This reconstructs metadata geometry only, never content authority.
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
    let StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    }) = selected.field_type
    else {
        return None;
    };
    (u64::from(offset).checked_add(8)?.checked_add(capacity)?
        <= u64::from(shape(structural_type, declarations)?.byte_size))
    .then_some((offset, capacity))
}

#[cfg(test)]
mod tests;
