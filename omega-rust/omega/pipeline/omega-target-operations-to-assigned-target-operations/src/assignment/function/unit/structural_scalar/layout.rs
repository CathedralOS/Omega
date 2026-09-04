//! Independent structural declaration, shape, path, and field-offset replay.

use std::collections::BTreeMap;

use omega_calling_conventions::ValueShape;
use psi_core::{IeeeFloatFormat, ScalarType, StructuralFieldId, StructuralTypeId};
use psi_terminal::{
    BindingRelevance, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(in crate::assignment::function) fn declaration_map(
    declarations: &[StructuralTypeDeclaration],
) -> Option<BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>> {
    let map = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    (map.len() == declarations.len()).then_some(map)
}

pub(in crate::assignment::function) fn structural_value_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<ValueShape> {
    structural_shape(
        structural_type,
        declarations,
        &mut BTreeMap::new(),
        &mut Vec::new(),
    )
}

pub(in crate::assignment::function) fn resolve_field_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<(StructuralTypeId, ValueShape, u32)> {
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let StructuralTypeShape::Record { fields } = &declarations.get(&structural_type)?.shape
        else {
            return None;
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for candidate in fields
            .iter()
            .filter(|candidate| physically_retained_field(candidate))
        {
            if matches!(candidate.field_type, StructuralFieldType::Erased { .. }) {
                continue;
            }
            let shape = field_shape(
                &candidate.field_type,
                declarations,
                &mut BTreeMap::new(),
                &mut Vec::new(),
            )?;
            local_offset = align(local_offset, u32::from(shape.alignment))?;
            if candidate.identity == *identity {
                let StructuralFieldType::Structural(nested) = candidate.field_type else {
                    return None;
                };
                selected = Some((nested, shape, local_offset));
                break;
            }
            local_offset = local_offset.checked_add(u32::from(shape.byte_size))?;
        }
        let (nested, shape, offset) = selected?;
        total_offset = total_offset.checked_add(offset)?;
        structural_type = nested;
        selected_shape = Some(shape);
    }
    Some((structural_type, selected_shape?, total_offset))
}

pub(in crate::assignment::function) fn resolve_projection_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<(StructuralTypeId, ValueShape, u32)> {
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    let mut cache = BTreeMap::new();
    let mut active = Vec::new();
    for segment in path {
        let (selected_type, shape, local_offset) = match segment {
            StructuralPathSegment::Field(_) => {
                resolve_field_path(structural_type, std::slice::from_ref(segment), declarations)?
            }
            StructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } =
                    declarations.get(&structural_type)?.shape
                else {
                    return None;
                };
                if *index >= length {
                    return None;
                }
                let shape = structural_shape(element, declarations, &mut cache, &mut active)?;
                let stride = align(u32::from(shape.byte_size), u32::from(shape.alignment))?;
                let offset = u64::from(stride)
                    .checked_mul(*index)
                    .and_then(|offset| u32::try_from(offset).ok())?;
                (element, shape, offset)
            }
        };
        total_offset = total_offset.checked_add(local_offset)?;
        structural_type = selected_type;
        selected_shape = Some(shape);
    }
    Some((structural_type, selected_shape?, total_offset))
}

pub(in crate::assignment::function) fn direct_scalar_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    scalar_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u32> {
    let StructuralTypeShape::Record { fields } = &declarations.get(&structural_type)?.shape else {
        return None;
    };
    let mut offset = 0_u32;
    for candidate in fields
        .iter()
        .filter(|candidate| physically_retained_field(candidate))
    {
        if matches!(candidate.field_type, StructuralFieldType::Erased { .. }) {
            continue;
        }
        let shape = field_shape(
            &candidate.field_type,
            declarations,
            &mut BTreeMap::new(),
            &mut Vec::new(),
        )?;
        offset = align(offset, u32::from(shape.alignment))?;
        if candidate.id == field {
            return (candidate.field_type == StructuralFieldType::Scalar(scalar_type))
                .then_some(offset);
        }
        offset = offset.checked_add(u32::from(shape.byte_size))?;
    }
    None
}

pub(in crate::assignment::function) fn scalar_field_offset_at_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    scalar_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<u32> {
    if !psi_terminal::is_bounded_structural_scalar_store_path(path) {
        return None;
    }
    let (field_owner, path_offset) = if path.is_empty() {
        (structural_type, 0)
    } else {
        let (nested, _, offset) = resolve_projection_path(structural_type, path, declarations)?;
        (nested, offset)
    };
    path_offset.checked_add(direct_scalar_field_offset(
        field_owner,
        field,
        scalar_type,
        declarations,
    )?)
}

fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    if let Some(shape) = cache.get(&structural_type) {
        return Some(*shape);
    }
    if active.contains(&structural_type) {
        return None;
    }
    active.push(structural_type);
    let shape = match &declarations.get(&structural_type)?.shape {
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => ValueShape::integer(1, 1),
        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            ValueShape::integer(size, size.next_power_of_two().min(8))
        }
        StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)) => {
            ValueShape::float(4)
        }
        StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)) => {
            ValueShape::float(8)
        }
        StructuralTypeShape::Record { fields } => {
            let mut size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields
                .iter()
                .filter(|field| physically_retained_field(field))
            {
                if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                    continue;
                }
                let field_shape = field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                size = align(size, u32::from(field_shape.alignment))?;
                size = size.checked_add(u32::from(field_shape.byte_size))?;
            }
            size = align(size, u32::from(alignment))?;
            ValueShape::integer(u16::try_from(size).ok()?, alignment)
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let element = structural_shape(*element, declarations, cache, active)?;
            let stride = align(u32::from(element.byte_size), u32::from(element.alignment))?;
            let size = u64::from(stride).checked_mul(*length)?;
            ValueShape::integer(u16::try_from(size).ok()?, element.alignment)
        }
        _ => return None,
    };
    active.pop();
    cache.insert(structural_type, shape);
    Some(shape)
}

fn field_shape(
    field: &StructuralFieldType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut Vec<StructuralTypeId>,
) -> Option<ValueShape> {
    match field {
        StructuralFieldType::Scalar(ScalarType::Boolean) => Some(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Some(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Some(ValueShape::float(8)),
        StructuralFieldType::Structural(nested) => {
            structural_shape(*nested, declarations, cache, active)
        }
        _ => None,
    }
}

fn physically_retained_field(field: &psi_terminal::StructuralFieldDeclaration) -> bool {
    field.relevance != BindingRelevance::Erased
        && !matches!(field.field_type, StructuralFieldType::Erased { .. })
}

fn align(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}
