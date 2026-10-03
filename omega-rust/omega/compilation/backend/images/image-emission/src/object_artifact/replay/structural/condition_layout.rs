//! Target-neutral structural layout replay.
//!
//! This module reconstructs exact aggregate shapes and projected offsets from
//! retained Terminal structural declarations. It does not select target
//! instructions or assign a new layout.

use calling_conventions::ValueShape;
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};

fn checked_align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|value| value / alignment * alignment)
}

/// Layout-visible record fields: authored relevance and physical carrier
/// erasure are independent, matching the lowering layout walk.
fn layout_field(field: &terminal_psi::StructuralFieldDeclaration) -> bool {
    !field.relevance.is_erased()
        && !matches!(
            field.field_type,
            terminal_psi::StructuralFieldType::Erased { .. }
        )
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
        // A nested established Binding contributes no condition payload.
        terminal_psi::StructuralTypeShape::FusedServiceBinding => ValueShape::integer(0, 1),
        // A reference field contains one address, not the referent's storage.
        // Its payload participates in enclosing offsets like any other field.
        terminal_psi::StructuralTypeShape::Reference { .. } => ValueShape::integer(8, 8),
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => {
            ValueShape::integer(1, 1)
        }
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            ValueShape::integer(u32::from(size), size.next_power_of_two().min(8))
        }
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary32,
        )) => ValueShape::float(4),
        terminal_psi::StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        )) => ValueShape::float(8),
        // First-class byte and element views are not Boolean-field aggregates
        // and have no native condition layout in this consumer.
        terminal_psi::StructuralTypeShape::ByteSequence(_)
        | terminal_psi::StructuralTypeShape::ElementView { .. } => return None,
        terminal_psi::StructuralTypeShape::Record { fields } => {
            let mut byte_size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields.iter().filter(|field| layout_field(field)) {
                let field_shape =
                    replay_structural_field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                byte_size = checked_align_up(byte_size, u32::from(field_shape.alignment))?
                    .checked_add(field_shape.byte_size)?;
            }
            byte_size = checked_align_up(byte_size, u32::from(alignment))?;
            // A record whose layout-visible children are all reference
            // carriers has the canonical empty aggregate shape.
            ValueShape::integer(byte_size, alignment)
        }
        terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
            if *length == 0 {
                return None;
            }
            let element = replay_structural_shape(*element, declarations, cache, active)?;
            let stride = checked_align_up(element.byte_size, u32::from(element.alignment))?;
            let byte_size = u64::from(stride)
                .checked_mul(*length)
                .and_then(|size| u32::try_from(size).ok())?;
            ValueShape::integer(byte_size, element.alignment)
        }
        terminal_psi::StructuralTypeShape::Sum { cases } => {
            replay_sum_layout(&[], cases, declarations, cache, active)?.shape
        }
        terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
            replay_sum_layout(fields, cases, declarations, cache, active)?.shape
        }
    };
    active.remove(&structural_type);
    cache.insert(structural_type, shape);
    Some(shape)
}

/// Reconstruct the conventional tag, common fields and overlaid payloads
/// from declarations. Geometry does not establish active-case permission.
fn replay_sum_layout(
    common: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &terminal_psi::StructuralTypeDeclaration,
    >,
    cache: &mut std::collections::BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut std::collections::BTreeSet<StructuralTypeId>,
) -> Option<calling_conventions::ConventionalSumLayout> {
    let common = common
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| replay_structural_field_shape(&field.field_type, declarations, cache, active))
        .collect::<Option<Vec<_>>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    replay_structural_field_shape(&field.field_type, declarations, cache, active)
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&common, &payloads).ok()
}

/// Reconstruct one complete native value shape from exact Terminal
/// declarations. Ranked object replay uses this independently of the public
/// target projection so a coordinated shape/call-plan rewrite cannot become
/// self-authorizing.
pub(crate) fn replay_structural_value_shape(
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

pub(crate) fn replay_structural_projection(
    mut structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
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
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        let (selected_type, shape, local_offset) = match segment {
            terminal_psi::StructuralPathSegment::Case(identity) => {
                let declaration = declarations.get(&structural_type)?;
                let (common, cases) = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::Sum { cases } => (&[][..], cases),
                    terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                        (fields.as_slice(), cases)
                    }
                    _ => return None,
                };
                let mut matching_cases = cases
                    .iter()
                    .enumerate()
                    .filter(|(_, case)| case.id == *identity);
                let (case_position, case) = matching_cases.next()?;
                if matching_cases.next().is_some() {
                    return None;
                }
                let Some(terminal_psi::StructuralPathSegment::Field(field_identity)) =
                    segments.next()
                else {
                    return None;
                };
                let mut matching_fields = case
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .filter(|(_, field)| field.identity == *field_identity);
                let (field_position, field) = matching_fields.next()?;
                if matching_fields.next().is_some() {
                    return None;
                }
                let terminal_psi::StructuralFieldType::Structural(selected) = field.field_type
                else {
                    return None;
                };
                let layout =
                    replay_sum_layout(common, cases, &declarations, &mut cache, &mut active)?;
                let field_offset = layout
                    .cases
                    .get(case_position)?
                    .fields
                    .get(field_position)?
                    .byte_offset;
                let shape =
                    replay_structural_shape(selected, &declarations, &mut cache, &mut active)?;
                (selected, shape, field_offset)
            }
            terminal_psi::StructuralPathSegment::Referent
            | terminal_psi::StructuralPathSegment::FixedByteRange { .. }
            | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => return None,
            terminal_psi::StructuralPathSegment::Field(identity) => {
                let declaration = declarations.get(&structural_type)?;
                let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape
                else {
                    return None;
                };
                let mut field_offset = 0_u32;
                let mut selected = None;
                for field in fields.iter().filter(|field| layout_field(field)) {
                    let shape = replay_structural_field_shape(
                        &field.field_type,
                        &declarations,
                        &mut cache,
                        &mut active,
                    )?;
                    field_offset = checked_align_up(field_offset, u32::from(shape.alignment))?;
                    if field.identity == *identity {
                        let terminal_psi::StructuralFieldType::Structural(nested) =
                            field.field_type
                        else {
                            return None;
                        };
                        selected = Some((nested, shape, field_offset));
                        break;
                    }
                    field_offset = field_offset.checked_add(shape.byte_size)?;
                }
                selected?
            }
            terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                let declaration = declarations.get(&structural_type)?;
                let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
                    declaration.shape
                else {
                    return None;
                };
                if *index >= length {
                    return None;
                }
                let shape =
                    replay_structural_shape(element, &declarations, &mut cache, &mut active)?;
                let stride = checked_align_up(shape.byte_size, u32::from(shape.alignment))?;
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

/// Replay a path carrying at least one runtime index: returns the selected
/// leaf type and shape, the static prefix byte offset, and one `(extent,
/// stride)` row per `RuntimeIndex` segment in path order. Static segments
/// resolve exactly as `replay_structural_projection` does; a runtime index
/// adds no static offset — its reach is `stride * (extent - 1)` beyond the
/// accumulated prefix at the largest in-bounds selector.
pub(crate) fn replay_runtime_projection(
    mut structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, ValueShape, u32, Vec<(u64, u32)>)> {
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    let mut elements = Vec::new();
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        if let terminal_psi::StructuralPathSegment::RuntimeIndex { .. } = segment {
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.id == structural_type)?;
            let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
                declaration.shape
            else {
                return None;
            };
            let shape = replay_structural_value_shape(element, declarations)?;
            let stride = checked_align_up(shape.byte_size, u32::from(shape.alignment))?;
            elements.push((length, stride));
            structural_type = element;
            selected_shape = Some(shape);
        } else {
            let pair;
            let projection = if matches!(segment, terminal_psi::StructuralPathSegment::Case(_)) {
                pair = [segment.clone(), segments.next()?.clone()];
                pair.as_slice()
            } else {
                std::slice::from_ref(segment)
            };
            let (selected_type, shape, local_offset) =
                replay_structural_projection(structural_type, projection, declarations)?;
            total_offset = total_offset.checked_add(local_offset)?;
            structural_type = selected_type;
            selected_shape = Some(shape);
        }
    }
    Some((structural_type, selected_shape?, total_offset, elements))
}

/// Replay a path whose last step names an inline bounded byte field: returns
/// the field's byte offset inside the root and its declared capacity. The
/// field has no catalog identity of its own, so this walks the record fields
/// directly; prefix segments resolve through records and fixed arrays exactly
/// as `replay_structural_projection` does.
pub(crate) fn replay_bounded_byte_field(
    mut structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(u32, u64)> {
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
    let (last, prefix) = path.split_last()?;
    let terminal_psi::StructuralPathSegment::Field(identity) = last else {
        return None;
    };
    let mut total_offset = 0_u32;
    for segment in prefix {
        let declaration = declarations.get(&structural_type)?;
        let (selected_type, local_offset) = match (segment, &declaration.shape) {
            (
                terminal_psi::StructuralPathSegment::Field(identity),
                terminal_psi::StructuralTypeShape::Record { fields },
            ) => {
                let mut field_offset = 0_u32;
                let mut selected = None;
                for field in fields.iter().filter(|field| layout_field(field)) {
                    let shape = replay_structural_field_shape(
                        &field.field_type,
                        &declarations,
                        &mut cache,
                        &mut active,
                    )?;
                    field_offset = checked_align_up(field_offset, u32::from(shape.alignment))?;
                    if field.identity == *identity {
                        let terminal_psi::StructuralFieldType::Structural(nested) =
                            field.field_type
                        else {
                            return None;
                        };
                        selected = Some((nested, field_offset));
                        break;
                    }
                    field_offset = field_offset.checked_add(shape.byte_size)?;
                }
                selected?
            }
            (
                terminal_psi::StructuralPathSegment::FixedIndex(index),
                terminal_psi::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                let shape =
                    replay_structural_shape(*element, &declarations, &mut cache, &mut active)?;
                let stride = checked_align_up(shape.byte_size, u32::from(shape.alignment))?;
                let offset = u64::from(stride)
                    .checked_mul(*index)
                    .and_then(|offset| u32::try_from(offset).ok())?;
                (*element, offset)
            }
            _ => return None,
        };
        total_offset = total_offset.checked_add(local_offset)?;
        structural_type = selected_type;
    }
    let declaration = declarations.get(&structural_type)?;
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut field_offset = 0_u32;
    for field in fields.iter().filter(|field| layout_field(field)) {
        let shape = replay_structural_field_shape(
            &field.field_type,
            &declarations,
            &mut cache,
            &mut active,
        )?;
        field_offset = checked_align_up(field_offset, u32::from(shape.alignment))?;
        if field.identity == *identity {
            let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) =
                field.field_type.byte_sequence_carrier()
            else {
                return None;
            };
            return total_offset
                .checked_add(field_offset)
                .map(|offset| (offset, capacity));
        }
        field_offset = field_offset.checked_add(shape.byte_size)?;
    }
    None
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
        terminal_psi::StructuralFieldType::Atomic(_) => replay_structural_field_shape(
            &terminal_psi::StructuralFieldType::Scalar(field_type.atomic_scalar_type()?),
            declarations,
            cache,
            active,
        ),
        terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean) => {
            Some(ValueShape::integer(1, 1))
        }
        terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(
                u32::from(size),
                size.next_power_of_two().min(16),
            ))
        }
        terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            let size = integer.bits().div_ceil(8);
            Some(ValueShape::integer(
                u32::from(size),
                size.next_power_of_two().min(16),
            ))
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
        terminal_psi::StructuralFieldType::ByteSequence(carrier)
        | terminal_psi::StructuralFieldType::PredicateByteSequence { carrier, .. } => {
            let byte_size: u32 = match carrier {
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 16,
                // A bounded view's extent lives in the declaration; the stored
                // descriptor is the bare referent pointer.
                terminal_psi::ByteSequenceCarrier::BoundedView { .. } => 8,
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

pub(crate) fn replay_boolean_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    declarations: &std::collections::BTreeMap<
        StructuralTypeId,
        &terminal_psi::StructuralTypeDeclaration,
    >,
) -> Option<(u32, ValueShape)> {
    let declaration = declarations.get(&structural_type)?;
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut cache = std::collections::BTreeMap::new();
    let mut active = std::collections::BTreeSet::new();
    let mut offset = 0_u32;
    for candidate in fields.iter().filter(|field| layout_field(field)) {
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
                terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean)
            )
            .then_some((
                offset,
                replay_structural_shape(structural_type, declarations, &mut cache, &mut active)?,
            ));
        }
        offset = offset.checked_add(shape.byte_size)?;
    }
    None
}

#[cfg(test)]
mod tests;
