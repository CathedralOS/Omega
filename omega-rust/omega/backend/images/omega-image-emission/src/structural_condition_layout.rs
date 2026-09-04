//! Target-neutral structural layout replay.
//!
//! This module reconstructs exact aggregate shapes and projected offsets from
//! retained Terminal structural declarations. It does not select target
//! instructions or assign a new layout.

use omega_calling_conventions::ValueShape;
use psi_core::{ScalarType, StructuralFieldId, StructuralTypeId};

fn checked_align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}

fn replay_structural_shape(
    structural_type: StructuralTypeId,
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
    cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut std::collections::BTreeSet<StructuralTypeId>,
) -> Option<ValueShape> {
    if let Some(shape) = cache.get(&structural_type) {
        return Some(*shape);
    }
    if !active.insert(structural_type) {
        return None;
    }
    let declaration = declarations.get(&structural_type)?;
    let shape = match &declaration.shape {
        psi_terminal::StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => {
            ValueShape::integer(1, 1)
        }
        psi_terminal::StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            ValueShape::integer(size, size.next_power_of_two().min(8))
        }
        psi_terminal::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            psi_core::IeeeFloatFormat::Binary32,
        )) => ValueShape::float(4),
        psi_terminal::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            psi_core::IeeeFloatFormat::Binary64,
        )) => ValueShape::float(8),
        // First-class byte views are not Boolean-field aggregates and have no
        // native condition layout in this consumer.
        psi_terminal::StructuralTypeShape::ByteSequence(_) => return None,
        psi_terminal::StructuralTypeShape::Record { fields } => {
            let mut byte_size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let field_shape =
                    replay_structural_field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                byte_size = checked_align_up(byte_size, u32::from(field_shape.alignment))?
                    .checked_add(u32::from(field_shape.byte_size))?;
            }
            byte_size = checked_align_up(byte_size, u32::from(alignment))?;
            if byte_size == 0 && !fields.is_empty() {
                return None;
            }
            ValueShape::integer(u16::try_from(byte_size).ok()?, alignment)
        }
        psi_terminal::StructuralTypeShape::FixedArray { element, length } => {
            if *length == 0 {
                return None;
            }
            let element = replay_structural_shape(*element, declarations, cache, active)?;
            let stride =
                checked_align_up(u32::from(element.byte_size), u32::from(element.alignment))?;
            let byte_size = u64::from(stride)
                .checked_mul(*length)
                .and_then(|size| u16::try_from(size).ok())?;
            ValueShape::integer(byte_size, element.alignment)
        }
        psi_terminal::StructuralTypeShape::Sum { .. }
        | psi_terminal::StructuralTypeShape::Mixed { .. } => return None,
    };
    active.remove(&structural_type);
    cache.insert(structural_type, shape);
    Some(shape)
}

/// Reconstruct one complete native value shape from exact Terminal
/// declarations. Ranked object replay uses this independently of the public
/// target projection so a coordinated shape/call-plan rewrite cannot become
/// self-authorizing.
pub(super) fn replay_structural_value_shape(
    structural_type: StructuralTypeId,
    declarations: &[psi_terminal::StructuralTypeDeclaration],
) -> Option<ValueShape> {
    let declarations = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    replay_structural_shape(structural_type, &declarations, &mut cache, &mut active)
}

pub(super) fn replay_structural_projection(
    mut structural_type: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
    declarations: &[psi_terminal::StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, ValueShape, u32)> {
    let declaration_count = declarations.len();
    let declarations = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<std::collections::BTreeMap<_, _>>();
    if declarations.len() != declaration_count {
        return None;
    }
    let mut cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let (selected_type, shape, local_offset) = match segment {
            psi_terminal::StructuralPathSegment::Field(identity) => {
                let declaration = declarations.get(&structural_type)?;
                let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape
                else {
                    return None;
                };
                let mut field_offset = 0_u32;
                let mut selected = None;
                for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                    let shape = replay_structural_field_shape(
                        &field.field_type,
                        &declarations,
                        &mut cache,
                        &mut active,
                    )?;
                    field_offset = checked_align_up(field_offset, u32::from(shape.alignment))?;
                    if field.identity == *identity {
                        let psi_terminal::StructuralFieldType::Structural(nested) =
                            field.field_type
                        else {
                            return None;
                        };
                        selected = Some((nested, shape, field_offset));
                        break;
                    }
                    field_offset = field_offset.checked_add(u32::from(shape.byte_size))?;
                }
                selected?
            }
            psi_terminal::StructuralPathSegment::FixedIndex(index) => {
                let declaration = declarations.get(&structural_type)?;
                let psi_terminal::StructuralTypeShape::FixedArray { element, length } =
                    declaration.shape
                else {
                    return None;
                };
                if *index >= length {
                    return None;
                }
                let shape =
                    replay_structural_shape(element, &declarations, &mut cache, &mut active)?;
                let stride =
                    checked_align_up(u32::from(shape.byte_size), u32::from(shape.alignment))?;
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

fn replay_structural_field_shape(
    field_type: &psi_terminal::StructuralFieldType,
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
    cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut std::collections::BTreeSet<StructuralTypeId>,
) -> Option<ValueShape> {
    match field_type {
        psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean) => {
            Some(ValueShape::integer(1, 1))
        }
        psi_terminal::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        psi_terminal::StructuralFieldType::Scalar(ScalarType::IeeeFloat(
            psi_core::IeeeFloatFormat::Binary32,
        )) => Some(ValueShape::float(4)),
        psi_terminal::StructuralFieldType::Scalar(ScalarType::IeeeFloat(
            psi_core::IeeeFloatFormat::Binary64,
        )) => Some(ValueShape::float(8)),
        psi_terminal::StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        psi_terminal::StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
        psi_terminal::StructuralFieldType::ByteSequence(carrier) => {
            let byte_size = match carrier {
                psi_terminal::ByteSequenceCarrier::BorrowedView => 16,
                psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    capacity.checked_add(8)?.try_into().ok()?
                }
            };
            Some(ValueShape::integer(byte_size, 8))
        }
        psi_terminal::StructuralFieldType::Structural(nested) => {
            replay_structural_shape(*nested, declarations, cache, active)
        }
        psi_terminal::StructuralFieldType::Erased { .. } => None,
    }
}

pub(super) fn replay_boolean_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &psi_terminal::StructuralTypeDeclaration,
    >,
) -> Option<(u32, ValueShape)> {
    let declaration = declarations.get(&structural_type)?;
    let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    let mut offset = 0_u32;
    for candidate in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let shape = replay_structural_field_shape(
            &candidate.field_type,
            declarations,
            &mut cache,
            &mut active,
        )?;
        offset = checked_align_up(offset, u32::from(shape.alignment))?;
        if candidate.id == field {
            return matches!(
                candidate.field_type,
                psi_terminal::StructuralFieldType::Scalar(ScalarType::Boolean)
            )
            .then_some((
                offset,
                replay_structural_shape(structural_type, declarations, &mut cache, &mut active)?,
            ));
        }
        offset = offset.checked_add(u32::from(shape.byte_size))?;
    }
    None
}
