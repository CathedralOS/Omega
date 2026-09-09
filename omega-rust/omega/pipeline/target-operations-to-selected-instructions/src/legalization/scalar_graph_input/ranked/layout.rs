//! Target-neutral structural layout replay.
//!
//! This module reconstructs exact aggregate shapes and projected offsets from
//! retained Terminal structural declarations. It does not select target
//! instructions or assign a new layout.

use calling_conventions::ValueShape;
use semantic_vocabulary::{ScalarType, StructuralTypeId};

fn checked_align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}

fn replay_structural_shape(
    structural_type: StructuralTypeId,
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &terminal_psi::StructuralTypeDeclaration,
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
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => {
            ValueShape::integer(1, 1)
        }
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            ValueShape::integer(size, size.next_power_of_two().min(8))
        }
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32,
        )) => ValueShape::float(4),
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        )) => ValueShape::float(8),
        // First-class byte views are not Boolean-field aggregates and have no
        // native condition layout in this consumer.
        terminal_psi::StructuralTypeShape::ByteSequence(_) => return None,
        terminal_psi::StructuralTypeShape::Record { fields } => {
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
        terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
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
        terminal_psi::StructuralTypeShape::Sum { .. }
        | terminal_psi::StructuralTypeShape::Mixed { .. } => {
            return None;
        }
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
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<ValueShape> {
    let declarations = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    replay_structural_shape(structural_type, &declarations, &mut cache, &mut active)
}

fn replay_structural_field_shape(
    field_type: &terminal_psi::StructuralFieldType,
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &terminal_psi::StructuralTypeDeclaration,
    >,
    cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut std::collections::BTreeSet<StructuralTypeId>,
) -> Option<ValueShape> {
    match field_type {
        terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean) => {
            Some(ValueShape::integer(1, 1))
        }
        terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(size, size.next_power_of_two().min(16)))
        }
        terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32,
        )) => Some(ValueShape::float(4)),
        terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        )) => Some(ValueShape::float(8)),
        terminal_psi::StructuralFieldType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32,
        ) => Some(ValueShape::float(4)),
        terminal_psi::StructuralFieldType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ) => Some(ValueShape::float(8)),
        terminal_psi::StructuralFieldType::ByteSequence(carrier) => {
            let byte_size = match carrier {
                terminal_psi::ByteSequenceCarrier::BorrowedView => 16,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    capacity.checked_add(8)?.try_into().ok()?
                }
            };
            Some(ValueShape::integer(byte_size, 8))
        }
        terminal_psi::StructuralFieldType::Structural(nested) => {
            replay_structural_shape(*nested, declarations, cache, active)
        }
        terminal_psi::StructuralFieldType::Erased { .. } => None,
    }
}
