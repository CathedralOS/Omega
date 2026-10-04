//! Native structural geometry retains each element declaration even at a zero
//! extent. Empty arrays have the canonical zero-byte ABI; their element graph
//! and every projected access remain independently checked.

use crate::LoweringError;
use calling_conventions::ValueShape;
use semantic_vocabulary::IeeeFloatFormat;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralPathSegment};
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

#[cfg(test)]
mod primitive_projection_tests;

/// Address arithmetic within one contiguous storage region. Crossing a
/// reference starts a new region; neither offsets nor indices commute with
/// the intervening pointer load.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct StorageProjection {
    pub(super) byte_offset: u32,
    pub(super) indices: Vec<(semantic_vocabulary::ValueId, u32)>,
}

/// Construction scratch for one primitive address, not a new target IR.
/// Every pointer slot is evaluated relative to the preceding loaded pointer
/// (the first uses the original root). The leaf is relative to the last one.
/// Target operations retain the original path so selection can independently
/// reconstruct these loads rather than trust flattened address metadata.
pub(super) struct PrimitiveProjection {
    pub(super) scalar: ScalarType,
    pub(super) pointer_slots: Vec<StorageProjection>,
    pub(super) leaf: StorageProjection,
}

/// Resolve a primitive address in path order. A reference declaration supplies
/// pointer geometry, not permission: the admitted operation must independently
/// establish live authority for every crossing. Origins never choose an address.
pub(super) fn primitive_leaf_projection(
    mut root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<PrimitiveProjection> {
    let mut pointer_slots = Vec::new();
    let mut segment_start = 0;
    for (position, segment) in path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let (reference, shape, byte_offset, indices) = runtime_projection(
            root,
            &path[segment_start..position],
            declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .ok()?;
        let StructuralTypeShape::Reference { referent, .. } = declarations.get(&reference)?.shape
        else {
            return None;
        };
        if shape != ValueShape::integer(8, 8) {
            return None;
        }
        pointer_slots.push(StorageProjection {
            byte_offset,
            indices,
        });
        root = referent;
        segment_start = position + 1;
    }
    let (scalar, byte_offset, indices) =
        contiguous_primitive_projection(root, &path[segment_start..], declarations)?;
    Some(PrimitiveProjection {
        scalar,
        pointer_slots,
        leaf: StorageProjection {
            byte_offset,
            indices,
        },
    })
}

/// Resolve the final contiguous region using the existing primitive endpoint
/// rules. In particular, bounded integer fields still require field-store proof
/// obligations; reference traversal does not turn them into primitive stores.
#[allow(clippy::type_complexity)]
fn contiguous_primitive_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)> {
    if let StructuralTypeShape::ByteSequence(carrier) = declarations.get(&root)?.shape {
        let scalar = terminal_semantics::primitive_projection_type(
            declarations.values().copied(),
            root,
            path,
        )?;
        let payload_offset: u32 = match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => 8,
            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 0,
        };
        // Geometry is relative to payload backing, not to a view's descriptor.
        // The sequence is the container; selecting its byte ends the path.
        return match path {
            [StructuralPathSegment::FixedIndex(index)] => Some((
                scalar,
                payload_offset.checked_add(u32::try_from(*index).ok()?)?,
                Vec::new(),
            )),
            [StructuralPathSegment::RuntimeIndex { index, .. }] => {
                Some((scalar, payload_offset, vec![(*index, 1)]))
            }
            _ => None,
        };
    }
    if let Some((StructuralPathSegment::Field(identity), carrier_path)) = path.split_last() {
        let (carrier, _, offset, runs) = runtime_projection(
            root,
            carrier_path,
            declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .ok()?;
        if let StructuralTypeShape::Record { fields } = &declarations.get(&carrier)?.shape {
            let field = fields
                .iter()
                .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
            let scalar = match field.field_type {
                StructuralFieldType::Scalar(scalar @ ScalarType::IeeeFloat(_)) => Some(scalar),
                StructuralFieldType::IeeeFloat(format) => Some(ScalarType::IeeeFloat(format)),
                _ => None,
            };
            if let Some(scalar) = scalar {
                let field_offset =
                    direct_scalar_field_offset(carrier, field.id, scalar, declarations).ok()?;
                return Some((scalar, offset.checked_add(field_offset)?, runs));
            }
        }
    }
    let (leaf, _, byte_offset, runs) = runtime_projection(
        root,
        path,
        declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .ok()?;
    match declarations.get(&leaf)?.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some((scalar, byte_offset, runs)),
        _ => None,
    }
}

/// Resolve the scalar a contiguous primitive read observes: the single-region
/// leaves `primitive_leaf_projection` resolves, plus a scalar field ending a
/// projection that selects a record element at run time (`cells[i].hp`). The
/// field's leaf type comes from `terminal_semantics::runtime_element_field`,
/// the carrier resolves like any projection, and the field lands at its
/// declared offset inside the record the carrier reaches, so the load is
/// `carrier offset + field offset + each selector times its stride`.
#[allow(clippy::type_complexity)]
pub(super) fn primitive_read_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)> {
    if let Some(leaf) = primitive_leaf_projection(root, path, declarations) {
        // This consumer emits one contiguous load. It cannot silently flatten
        // pointer crossings; a dereferencing read needs its own admitted load
        // sequence and independent native replay.
        if !leaf.pointer_slots.is_empty() {
            return None;
        }
        return Some((leaf.scalar, leaf.leaf.byte_offset, leaf.leaf.indices));
    }
    let (scalar, record) =
        terminal_semantics::runtime_element_field(declarations.values().copied(), root, path)?;
    let (StructuralPathSegment::Field(identity), carrier) = path.split_last()? else {
        return None;
    };
    let (carrier_type, _, carrier_offset, runs) = runtime_projection(
        root,
        carrier,
        declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .ok()?;
    let StructuralTypeShape::Record { fields } = &record.shape else {
        return None;
    };
    let field = fields.iter().find(|field| field.identity == *identity)?;
    if carrier_type != record.id {
        return None;
    }
    let field_offset =
        direct_scalar_field_offset(carrier_type, field.id, scalar, declarations).ok()?;
    Some((scalar, carrier_offset.checked_add(field_offset)?, runs))
}

mod scalar_fields;
pub(super) use scalar_fields::{
    direct_boolean_field_offset, direct_integer_field_offset, direct_scalar_field_offset,
};

/// Existing copy metadata describes the root array, not the final index in a
/// nested path. Record-root projections have no root array metadata.
pub(super) fn root_array_projection_metadata(
    root: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(Option<u64>, Option<u32>), LoweringError> {
    let declaration = declarations
        .get(&root)
        .ok_or(LoweringError::UnknownStructuralType(root))?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return Ok((None, None));
    };
    let shape = structural_shape(element, declarations, cache, active)?;
    let stride = checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))
        .ok_or(LoweringError::StructuralTypeTooLarge(root))?;
    Ok((Some(length), Some(stride)))
}

pub(crate) fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, LoweringError> {
    if let Some(shape) = cache.get(&structural_type) {
        return Ok(*shape);
    }
    if !active.insert(structural_type) {
        return Err(LoweringError::RecursiveStructuralType(structural_type));
    }
    let result = (|| {
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        match &declaration.shape {
            // Established Fused bindings have no payload. This describes only
            // geometry; their marker is not an empty-record constructor.
            StructuralTypeShape::FusedServiceBinding => Ok(ValueShape::integer(0, 1)),
            // All supported native ABIs use 64-bit addresses. The carrier
            // stores the referent's address, never its contents; custody is
            // checked separately. Nested reference leaves use the same layout
            // as whole results, so enclosing record offsets remain coherent.
            StructuralTypeShape::Reference { .. } => Ok(ValueShape::integer(8, 8)),
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean) => {
                Ok(ValueShape::integer(1, 1))
            }
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)) => {
                let byte_size = integer.bits().div_ceil(8);
                Ok(ValueShape::integer(
                    u32::from(byte_size),
                    byte_size.next_power_of_two().min(8),
                ))
            }
            StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                IeeeFloatFormat::Binary32,
            )) => Ok(ValueShape::float(4)),
            StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                IeeeFloatFormat::Binary64,
            )) => Ok(ValueShape::float(8)),
            StructuralTypeShape::ByteSequence(carrier) => {
                byte_sequence_shape(*carrier, structural_type)
            }
            // A borrowed element view is a base+extent descriptor, the same
            // two-word carrier as `ByteSequence::BorrowedView`.
            StructuralTypeShape::ElementView { .. } => Ok(ValueShape::integer(16, 8)),
            StructuralTypeShape::Record { fields } => {
                let mut byte_size = 0_u32;
                let mut alignment = 1_u16;
                for field in fields {
                    // Authored relevance and physical carrier erasure are
                    // independent: a Fused service remains semantically relevant.
                    if field.relevance.is_erased()
                        || matches!(field.field_type, StructuralFieldType::Erased { .. })
                    {
                        continue;
                    }
                    let field_shape = structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        cache,
                        active,
                    )?;
                    alignment = alignment.max(field_shape.alignment);
                    byte_size = checked_align_up_u32(byte_size, u32::from(field_shape.alignment))
                        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                    byte_size = byte_size
                        .checked_add(field_shape.byte_size)
                        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                }
                byte_size = checked_align_up_u32(byte_size, u32::from(alignment))
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                Ok(ValueShape::integer(byte_size, alignment))
            }
            StructuralTypeShape::FixedArray { element, length } => {
                // The complete primitive-array chain is still checked below
                // empty dimensions. Its canonical zero-byte ABI must not need
                // an addressable element layout or overflow an unused extent.
                if let Ok((_, 0, shape)) =
                    super::control_flow::scalar_arrays::shape(structural_type, declarations)
                {
                    return Ok(shape);
                }
                let element = structural_shape(*element, declarations, cache, active)?;
                let stride = checked_align_up_u32(element.byte_size, u32::from(element.alignment))
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                let byte_size = u64::from(stride)
                    .checked_mul(*length)
                    .and_then(|size| u32::try_from(size).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
                Ok(ValueShape::integer(
                    byte_size,
                    if *length == 0 { 1 } else { element.alignment },
                ))
            }
            StructuralTypeShape::Sum { cases } => conventional_sum_layout_from_parts(
                structural_type,
                &[],
                cases,
                declarations,
                cache,
                active,
            )
            .map(|layout| layout.shape),
            StructuralTypeShape::Mixed { fields, cases } => conventional_sum_layout_from_parts(
                structural_type,
                fields,
                cases,
                declarations,
                cache,
                active,
            )
            .map(|layout| layout.shape),
        }
    })();
    active.remove(&structural_type);
    let shape = result?;
    cache.insert(structural_type, shape);
    Ok(shape)
}

pub(super) fn structural_parameter_shape(
    referent: ValueShape,
    access: StructuralAccess,
) -> ValueShape {
    match access {
        StructuralAccess::Owned => referent,
        StructuralAccess::SharedBorrow
        | StructuralAccess::MutableBorrow
        | StructuralAccess::WriteOnlyBorrow => {
            ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
        }
    }
}

pub(super) fn structural_sum_layout(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    match &declaration.shape {
        StructuralTypeShape::Sum { cases } => conventional_sum_layout_from_parts(
            structural_type,
            &[],
            cases,
            declarations,
            cache,
            active,
        ),
        StructuralTypeShape::Mixed { fields, cases } => conventional_sum_layout_from_parts(
            structural_type,
            fields,
            cases,
            declarations,
            cache,
            active,
        ),
        _ => Err(LoweringError::UnsupportedStructuralSum(structural_type)),
    }
}

fn conventional_sum_layout_from_parts(
    structural_type: StructuralTypeId,
    common_fields: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<calling_conventions::ConventionalSumLayout, LoweringError> {
    if cases.is_empty() {
        return Err(LoweringError::EmptyStructuralType(structural_type));
    }
    let common = common_fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| {
            structural_field_shape(
                &field.field_type,
                structural_type,
                declarations,
                cache,
                active,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        cache,
                        active,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
        .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))
}

fn structural_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, LoweringError> {
    if matches!(field, StructuralFieldType::Atomic(_)) && field.atomic_scalar_type().is_none() {
        return Err(LoweringError::UnknownStructuralType(owner));
    }
    match field {
        StructuralFieldType::RefinedScalar(refinement) => structural_field_shape(
            &StructuralFieldType::Scalar(refinement.carrier()),
            owner,
            declarations,
            cache,
            active,
        ),
        StructuralFieldType::Scalar(ScalarType::Boolean)
        | StructuralFieldType::Atomic(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
        | StructuralFieldType::Atomic(ScalarType::Integer(integer)) => {
            let size = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(
                u32::from(size),
                size.next_power_of_two().min(16),
            ))
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            let size = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(
                u32::from(size),
                size.next_power_of_two().min(16),
            ))
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::Atomic(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::Atomic(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(ValueShape::float(8)),
        StructuralFieldType::ByteSequence(carrier)
        | StructuralFieldType::PredicateByteSequence { carrier, .. } => {
            byte_sequence_shape(*carrier, owner)
        }
        StructuralFieldType::Structural(nested) => {
            structural_shape(*nested, declarations, cache, active)
        }
        StructuralFieldType::Erased { .. } => {
            // Callers filter erased fields before requesting runtime shape.
            Err(LoweringError::UnknownStructuralType(owner))
        }
    }
}

pub(super) fn resolve_structural_field_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    field_path(
        structural_type,
        path,
        declarations,
        cache,
        active,
        StructuralFieldType::canonical_leaf_shape,
    )
}

fn field_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaf_shape: fn(&StructuralFieldType) -> Option<StructuralTypeShape>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let root_type = structural_type;
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
                continue;
            }
            let field_shape = structural_field_shape(
                &field.field_type,
                structural_type,
                declarations,
                cache,
                active,
            )?;
            local_offset = checked_align_up_u32(local_offset, u32::from(field_shape.alignment))
                .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
            if field.identity == *identity {
                let field_type = match &field.field_type {
                    StructuralFieldType::Structural(nested) => *nested,
                    leaf => {
                        let shape = leaf_shape(leaf);
                        let field_type = match (shape, leaf.byte_sequence_carrier()) {
                            (Some(shape), _) => declarations
                                .iter()
                                .find(|(_, declaration)| declaration.shape == shape)
                                .map(|(id, _)| *id),
                            // A byte-carrier field names its collection by
                            // the carrier itself; the canonical leaf shape
                            // cannot spell it.
                            (None, Some(carrier)) => {
                                terminal_semantics::byte_carrier_field_declaration(
                                    declarations.values().copied(),
                                    carrier,
                                )
                                .map(|declaration| declaration.id)
                            }
                            (None, None) => None,
                        };
                        field_type.ok_or(LoweringError::UnknownStructuralType(structural_type))?
                    }
                };
                selected = Some((field_type, field_shape, local_offset));
                break;
            }
            local_offset = local_offset
                .checked_add(field_shape.byte_size)
                .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        }
        let (field_type, field_shape, field_offset) =
            selected.ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        total_offset = total_offset
            .checked_add(field_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        structural_type = field_type;
        selected_shape = Some(field_shape);
    }
    selected_shape
        .map(|shape| (structural_type, shape, total_offset))
        .ok_or(LoweringError::UnknownStructuralType(root_type))
}

/// A byte-sequence field is a leaf carrier, not a structural type: the path's
/// last segment names a record field carrying `ByteSequence` directly. Resolve
/// only the field's byte offset and carrier; the storage stays the caller's.
/// `Ok(None)` means the path does not end in such a field; `Err` means the
/// prefix itself is malformed.
fn byte_sequence_field_geometry(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<(u32, terminal_psi::ByteSequenceCarrier)>, LoweringError> {
    let Some((StructuralPathSegment::Field(identity), prefix)) = path.split_last() else {
        return Ok(None);
    };
    let (parent_type, _, parent_offset) = if prefix.is_empty() {
        (
            structural_type,
            structural_shape(structural_type, declarations, cache, active)?,
            0,
        )
    } else {
        resolve_structural_projection_path(structural_type, prefix, declarations, cache, active)?
    };
    let declaration = declarations
        .get(&parent_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(parent_type))?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Ok(None);
    };
    let mut local_offset = 0_u32;
    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let field_shape = match structural_field_shape(
            &field.field_type,
            parent_type,
            declarations,
            cache,
            active,
        ) {
            // Erased carriers occupy no storage; the shape helper declines them.
            Ok(shape) => shape,
            Err(_) if matches!(field.field_type, StructuralFieldType::Erased { .. }) => continue,
            Err(error) => return Err(error),
        };
        local_offset = checked_align_up_u32(local_offset, u32::from(field_shape.alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(parent_type))?;
        if field.identity == *identity {
            return match field.field_type {
                StructuralFieldType::ByteSequence(carrier)
                | StructuralFieldType::PredicateByteSequence { carrier, .. } => Ok(Some((
                    parent_offset
                        .checked_add(local_offset)
                        .ok_or(LoweringError::StructuralTypeTooLarge(parent_type))?,
                    carrier,
                ))),
                _ => Ok(None),
            };
        }
        local_offset = local_offset
            .checked_add(field_shape.byte_size)
            .ok_or(LoweringError::StructuralTypeTooLarge(parent_type))?;
    }
    Ok(None)
}

/// Bounded inline byte storage is a field, not a structural type: the path's
/// last segment names a record field carrying `ByteSequence(BoundedOwned)`
/// directly. Resolve only the field's offset and declared capacity; the live
/// length word at that offset and the bytes after it stay the caller's storage.
/// `Ok(None)` means the path does not end in such a field; `Err` means the
/// prefix itself is malformed.
pub(super) fn bounded_byte_field_geometry(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<(u32, u64)>, LoweringError> {
    byte_sequence_field_geometry(structural_type, path, declarations, cache, active).map(
        |geometry| {
            geometry.and_then(|(offset, carrier)| match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                    Some((offset, capacity))
                }
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => None,
            })
        },
    )
}

/// A stored borrowed-view descriptor field (`ByteSequence(BorrowedView)`)
/// likewise carries no leaf type identity; the formal's declared descriptor
/// type supplies it at the boundary. Resolve only its byte offset.
/// `Ok(None)` means the path does not end in such a field; `Err` means the
/// prefix itself is malformed.
pub(super) fn borrowed_view_field_offset(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<Option<u32>, LoweringError> {
    byte_sequence_field_geometry(structural_type, path, declarations, cache, active).map(
        |geometry| {
            geometry.and_then(|(offset, carrier)| match carrier {
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => Some(offset),
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
            })
        },
    )
}

/// Case payloads overlay storage, but their case and field identities remain
/// part of the copy contract. Resolve those identities before using layout.
pub(super) fn case_projection(
    root: StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Segment;
    let invalid = || LoweringError::UnsupportedStructuralSum(root);
    let endpoint =
        terminal_semantics::canonical_case_leaf_type(declarations.values().copied(), root, path)
            .ok_or_else(invalid)?;
    let mut current = root;
    let mut offset = 0u32;
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        let shape = &declarations.get(&current).ok_or_else(invalid)?.shape;
        let (next, displacement) = match segment {
            Segment::Case(identity) => {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    shape
                else {
                    return Err(invalid());
                };
                let case_position = cases
                    .iter()
                    .position(|case| case.id == *identity)
                    .ok_or_else(invalid)?;
                let Some(Segment::Field(field)) = segments.next() else {
                    return Err(invalid());
                };
                let (field_position, declaration) = cases[case_position]
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, declaration)| declaration.id == *field)
                    .ok_or_else(invalid)?;
                let StructuralFieldType::Structural(next) = declaration.field_type else {
                    return Err(invalid());
                };
                let layout = structural_sum_layout(current, declarations, cache, active)?;
                (
                    next,
                    layout.cases[case_position].fields[field_position].byte_offset,
                )
            }
            Segment::Field(identity) => {
                let (StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. }) = shape
                else {
                    return Err(invalid());
                };
                let (field_position, field) = fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.id == *identity)
                    .ok_or_else(invalid)?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return Err(invalid());
                };
                let displacement = if matches!(shape, StructuralTypeShape::Mixed { .. }) {
                    structural_sum_layout(current, declarations, cache, active)?.common_fields
                        [field_position]
                        .byte_offset
                } else {
                    resolve_structural_field_path(
                        current,
                        &[StructuralPathSegment::Field(field.identity.clone())],
                        declarations,
                        cache,
                        active,
                    )?
                    .2
                };
                (next, displacement)
            }
            Segment::FixedIndex(index) => {
                let (next, _, displacement) = resolve_structural_projection_path(
                    current,
                    &[StructuralPathSegment::FixedIndex(*index)],
                    declarations,
                    cache,
                    active,
                )?;
                (next, displacement)
            }
        };
        current = next;
        offset = offset.checked_add(displacement).ok_or_else(invalid)?;
    }
    if current != endpoint {
        return Err(invalid());
    }
    Ok((
        endpoint,
        structural_shape(endpoint, declarations, cache, active)?,
        offset,
    ))
}

pub(super) fn resolve_structural_projection_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    projection_path(
        structural_type,
        path,
        declarations,
        cache,
        active,
        StructuralFieldType::canonical_leaf_shape,
    )
}

fn projection_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaf_shape: fn(&StructuralFieldType) -> Option<StructuralTypeShape>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let root_type = structural_type;
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        let (selected_type, shape, local_offset) = match segment {
            StructuralPathSegment::Case(case) => {
                let invalid = || LoweringError::UnsupportedStructuralSum(structural_type);
                let declaration = declarations.get(&structural_type).ok_or_else(invalid)?;
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return Err(invalid());
                };
                let case_position = cases
                    .iter()
                    .position(|candidate| candidate.id == *case)
                    .ok_or_else(invalid)?;
                let Some(StructuralPathSegment::Field(identity)) = segments.next() else {
                    return Err(invalid());
                };
                let (field_position, field) = cases[case_position]
                    .fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.identity == *identity)
                    .ok_or_else(invalid)?;
                let StructuralFieldType::Structural(selected) = field.field_type else {
                    return Err(invalid());
                };
                let layout = structural_sum_layout(structural_type, declarations, cache, active)?;
                (
                    selected,
                    structural_shape(selected, declarations, cache, active)?,
                    layout.cases[case_position].fields[field_position].byte_offset,
                )
            }
            StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. }
            | StructuralPathSegment::RuntimeIndex { .. } => {
                return Err(LoweringError::UnsupportedStructuralReference(
                    structural_type,
                ));
            }
            StructuralPathSegment::Field(_) => field_path(
                structural_type,
                std::slice::from_ref(segment),
                declarations,
                cache,
                active,
                leaf_shape,
            )?,
            StructuralPathSegment::FixedIndex(index) => {
                let declaration = declarations
                    .get(&structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, length } => {
                        if *index >= *length {
                            return Err(LoweringError::UnknownStructuralType(structural_type));
                        }
                        *element
                    }
                    // A view's storage is element-contiguous like the
                    // array's; its bound lives in the verifier's extent
                    // obligation rather than a literal type length.
                    StructuralTypeShape::ElementView { element } => *element,
                    _ => return Err(LoweringError::UnknownStructuralType(structural_type)),
                };
                let shape = structural_shape(element, declarations, cache, active)?;
                let stride = checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))
                    .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
                let offset = u64::from(stride)
                    .checked_mul(*index)
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
                (element, shape, offset)
            }
        };
        total_offset = total_offset
            .checked_add(local_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        structural_type = selected_type;
        selected_shape = Some(shape);
    }
    // The empty path is the identity projection, not a missing declaration.
    // Resolve its root through the same layout validation as every child.
    let shape = match selected_shape {
        Some(shape) => shape,
        None => structural_shape(structural_type, declarations, cache, active)?,
    };
    Ok((structural_type, shape, total_offset))
}

type RuntimeProjection = (
    StructuralTypeId,
    ValueShape,
    u32,
    Vec<(semantic_vocabulary::ValueId, u32)>,
);

/// Resolve a projection that may traverse `RuntimeIndex` segments at any
/// depth — a leaf copy's extent, a primitive leaf, or a field store's
/// carrier. Static segments before, between, and after the dynamic segments
/// fold into the returned `byte_offset`; each dynamic segment contributes
/// `(index, stride)` so the access scales every runtime operand into the
/// same address in path order. Each segment's bound is the obligation the
/// terminal verifier discharged; only array shape is read.
#[allow(clippy::type_complexity)]
pub(super) fn runtime_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<
    (
        StructuralTypeId,
        ValueShape,
        u32,
        Vec<(semantic_vocabulary::ValueId, u32)>,
    ),
    LoweringError,
> {
    runtime_projection_with_leaf_shape(
        structural_type,
        path,
        declarations,
        cache,
        active,
        StructuralFieldType::canonical_leaf_shape,
    )
}

/// A snapshot may copy a declared byte field without making that field an
/// ordinary borrow or call projection. Qualification custody stays with the
/// copy operation's independent source replay.
pub(super) fn runtime_leaf_copy_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<RuntimeProjection, LoweringError> {
    runtime_projection_with_leaf_shape(
        structural_type,
        path,
        declarations,
        cache,
        active,
        StructuralFieldType::leaf_copy_shape,
    )
}

fn runtime_projection_with_leaf_shape(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaf_shape: fn(&StructuralFieldType) -> Option<StructuralTypeShape>,
) -> Result<RuntimeProjection, LoweringError> {
    let mut structural_type = structural_type;
    let mut byte_offset = 0u32;
    let mut indices = Vec::new();
    let mut segment_start = 0usize;
    for (position, segment) in path.iter().enumerate() {
        let StructuralPathSegment::RuntimeIndex { index, .. } = segment else {
            continue;
        };
        if position > segment_start {
            let (container, _shape, run_offset) = projection_path(
                structural_type,
                &path[segment_start..position],
                declarations,
                cache,
                active,
                leaf_shape,
            )?;
            structural_type = container;
            byte_offset = byte_offset
                .checked_add(run_offset)
                .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        }
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        let element = match &declaration.shape {
            StructuralTypeShape::FixedArray { element, .. }
            // A view carries no literal length: the selector's bound is the
            // operation's own obligation against the stored extent.
            | StructuralTypeShape::ElementView { element } => *element,
            _ => {
                return Err(LoweringError::UnsupportedStructuralReference(
                    structural_type,
                ));
            }
        };
        let element_shape = structural_shape(element, declarations, cache, active)?;
        let stride =
            checked_align_up_u32(element_shape.byte_size, u32::from(element_shape.alignment))
                .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        indices.push((*index, stride));
        structural_type = element;
        segment_start = position + 1;
    }
    let (endpoint, shape, tail_offset) = if segment_start == path.len() {
        let shape = structural_shape(structural_type, declarations, cache, active)?;
        (structural_type, shape, 0)
    } else {
        projection_path(
            structural_type,
            &path[segment_start..],
            declarations,
            cache,
            active,
            leaf_shape,
        )?
    };
    let byte_offset = byte_offset
        .checked_add(tail_offset)
        .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
    Ok((endpoint, shape, byte_offset, indices))
}

pub(super) fn byte_sequence_shape(
    carrier: terminal_psi::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, LoweringError> {
    let byte_size = match carrier {
        // Current native targets are 64-bit. The semantic carrier
        // deliberately does not retain the physical descriptor fields.
        terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 16_u64,
        // A bounded view's extent lives in the declaration, so the descriptor
        // is the bare referent pointer.
        terminal_psi::ByteSequenceCarrier::BoundedView { .. } => 8_u64,
        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => capacity
            .checked_add(8)
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?,
    };
    Ok(ValueShape::integer(
        u32::try_from(byte_size)
            .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
        8,
    ))
}

pub(super) fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_record_array_layout_preserves_element_graph_checks() {
        let root = StructuralTypeId::new(1).unwrap();
        let element = StructuralTypeId::new(2).unwrap();
        let mut declarations = [
            StructuralTypeDeclaration {
                id: root,
                identity: "EmptyRecords".into(),
                shape: StructuralTypeShape::FixedArray { element, length: 0 },
            },
            StructuralTypeDeclaration {
                id: element,
                identity: "Element".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ];
        let indexed = declarations.iter().map(|value| (value.id, value)).collect();
        assert_eq!(
            structural_shape(root, &indexed, &mut BTreeMap::new(), &mut BTreeSet::new()).unwrap(),
            ValueShape::integer(0, 1)
        );
        for invalid in [root, StructuralTypeId::new(99).unwrap()] {
            declarations[0].shape = StructuralTypeShape::FixedArray {
                element: invalid,
                length: 0,
            };
            let indexed = declarations.iter().map(|value| (value.id, value)).collect();
            assert!(
                structural_shape(root, &indexed, &mut BTreeMap::new(), &mut BTreeSet::new())
                    .is_err()
            );
        }
    }

    #[test]
    fn empty_projection_retains_root_layout_including_zero_extent() {
        let element = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        for length in [0, 2] {
            let declarations = [
                StructuralTypeDeclaration {
                    id: element,
                    identity: "Boolean".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                },
                StructuralTypeDeclaration {
                    id: array,
                    identity: "Array".into(),
                    shape: StructuralTypeShape::FixedArray { element, length },
                },
            ];
            let indexed = declarations.iter().map(|value| (value.id, value)).collect();
            assert_eq!(
                resolve_structural_projection_path(
                    array,
                    &[],
                    &indexed,
                    &mut BTreeMap::new(),
                    &mut BTreeSet::new(),
                )
                .unwrap(),
                (array, ValueShape::integer(length as u32, 1), 0),
            );
            let unknown = StructuralTypeId::new(3).unwrap();
            assert!(matches!(
                resolve_structural_projection_path(
                    unknown, &[], &indexed, &mut BTreeMap::new(), &mut BTreeSet::new(),
                ),
                Err(LoweringError::UnknownStructuralType(identity)) if identity == unknown
            ));
        }
    }

    #[test]
    fn array_extents_above_64k_remain_exact_and_u32_overflow_rejects() {
        let element = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        for (length, expected) in [(65_536, Some(65_536)), (u64::from(u32::MAX) + 1, None)] {
            let declarations = [
                StructuralTypeDeclaration {
                    id: element,
                    identity: "Byte".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                },
                StructuralTypeDeclaration {
                    id: array,
                    identity: "Array".into(),
                    shape: StructuralTypeShape::FixedArray { element, length },
                },
            ];
            let indexed = declarations.iter().map(|value| (value.id, value)).collect();
            let result =
                structural_shape(array, &indexed, &mut BTreeMap::new(), &mut BTreeSet::new());
            match expected {
                Some(size) => assert_eq!(result.unwrap(), ValueShape::integer(size, 1)),
                None => assert!(
                    matches!(result, Err(LoweringError::StructuralTypeTooLarge(value)) if value == array)
                ),
            }
        }
    }
}
