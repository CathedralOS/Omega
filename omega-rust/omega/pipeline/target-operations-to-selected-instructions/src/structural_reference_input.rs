//! Input-only reconstruction of borrowed referent geometry.
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

/// Reconstruct a whole primitive write without inventing a carrier record or field.
pub(crate) fn primitive_store(
    destination: &terminal_psi::StructuralParameterDeclaration,
    scalar: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<u8> {
    if destination.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
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
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == destination.structural_type);
    let declaration = matches.next()?;
    if matches.next().is_some() || declaration.shape != StructuralTypeShape::PrimitiveScalar(scalar)
    {
        return None;
    }
    u8::try_from(scalar_shape(scalar)?.byte_size).ok()
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
    if !parameter.qualifications.is_empty() || !parameter.projected_qualifications.is_empty() {
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
        return primitive_array_shape(parameter.structural_type, declarations);
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
            StructuralMultiplicity::Unrestricted,
        ) => Some(ValueShape::borrowed_reference(
            referent.byte_size,
            referent.alignment,
        )),
        _ => None,
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
    if source.place != argument.place
        || source.access != terminal_psi::StructuralAccess::MutableBorrow
        || argument.access != terminal_psi::StructuralAccess::MutableBorrow
        || source.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !source.qualifications.is_empty()
        || !source.projected_qualifications.is_empty()
        || !argument
            .path
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
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
    let (array_type, offset) = project(source.structural_type, &argument.path, declarations)?;
    let StructuralTypeShape::FixedArray { element, length } = declarations
        .iter()
        .find(|declaration| declaration.id == array_type)?
        .shape
    else {
        return None;
    };
    if length == 0 || !declarations.iter().any(|declaration| declaration.id == element
        && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address()))
    {
        return None;
    }
    let root = shape(source.structural_type, declarations)?;
    (u64::from(offset).checked_add(length)? <= u64::from(root.byte_size))
        .then_some((offset, length))
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
        StructuralTypeShape::Record { fields } => {
            let mut bytes = 0;
            let mut alignment = 1;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
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
        StructuralFieldType::IeeeFloat(format) => scalar_shape(ScalarType::IeeeFloat(*format)),
        StructuralFieldType::Structural(nested) => shape_inner(*nested, declarations, active),
        _ => None,
    }
}

pub(crate) fn project(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
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
                StructuralPathSegment::FixedIndex(position),
                StructuralTypeShape::FixedArray { element, length },
            ) if position < length => {
                let element_shape = shape(*element, declarations)?;
                let stride = align(u32::from(element_shape.byte_size), element_shape.alignment)?;
                offset = offset
                    .checked_add(u32::try_from(u64::from(stride).checked_mul(*position)?).ok()?)?;
                carrier = *element;
            }
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let mut field_offset = 0;
                let mut found = None;
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let layout = field_shape(&field.field_type, declarations, &mut Vec::new())?;
                    field_offset = align(field_offset, layout.alignment)?;
                    if field.identity == *identity {
                        let StructuralFieldType::Structural(nested) = field.field_type else {
                            return None;
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
    let (carrier, carrier_offset) = project(root, path, declarations)?;
    let StructuralTypeShape::Record { fields } = &declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let mut offset = 0;
    for candidate in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let layout = field_shape(&candidate.field_type, declarations, &mut Vec::new())?;
        offset = align(offset, layout.alignment)?;
        if candidate.id == field {
            let matches_type = match candidate.field_type {
                StructuralFieldType::Scalar(actual) => actual == scalar,
                StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(format) == scalar,
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
