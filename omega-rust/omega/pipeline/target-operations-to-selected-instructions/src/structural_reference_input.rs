//! Input-only reconstruction of borrowed referent geometry.
use calling_conventions::ValueShape;
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

fn align(value: u32, alignment: u16) -> Option<u32> {
    let alignment = u32::from(alignment);
    Some(value.checked_add(alignment.checked_sub(1)?)? / alignment * alignment)
}

pub(crate) fn scalar_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) if matches!(integer.bits(), 8 | 16 | 32 | 64) => {
            let bytes = integer.bits() / 8;
            Some(ValueShape::integer(bytes, bytes))
        }
        _ => None,
    }
}

pub(crate) fn shape(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ValueShape> {
    shape_inner(root, declarations, &mut Vec::new())
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
            if candidate.field_type != StructuralFieldType::Scalar(scalar) {
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
