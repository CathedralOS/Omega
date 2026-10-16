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

/// Select borrowed backing after each stored reference crossing. Offsets are
/// relative to the last referent, never to a descriptor containing its pointer.
/// Ordinary and foreign calls use the same geometry; call custody establishes
/// liveness independently and the selected ABI determines pointer transport.
#[allow(clippy::type_complexity)]
pub(super) fn reference_referent_projection(
    mut root: StructuralTypeId,
    mut path: &[StructuralPathSegment],
    requested: StructuralAccess,
    expected: Option<StructuralTypeId>,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<
    (
        StructuralTypeId,
        ValueShape,
        u32,
        Vec<(semantic_vocabulary::ValueId, u32)>,
    ),
    LoweringError,
> {
    let mut shapes = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut runs = Vec::new();
    while let Some(crossing) = path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)
    {
        let (carrier, _, _, indices) = runtime_projection(
            root,
            &path[..crossing],
            declarations,
            &mut shapes,
            &mut active,
        )?;
        let Some(StructuralTypeShape::Reference { referent, access }) = declarations
            .get(&carrier)
            .map(|declaration| &declaration.shape)
        else {
            return Err(LoweringError::UnsupportedStructuralReference(root));
        };
        if !(*access == requested
            || *access == StructuralAccess::MutableBorrow
                && matches!(
                    requested,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
        {
            return Err(LoweringError::UnsupportedStructuralReference(root));
        }
        runs.extend(indices);
        root = *referent;
        path = &path[crossing + 1..];
        if *access == StructuralAccess::WriteOnlyBorrow
            && path.contains(&StructuralPathSegment::Referent)
        {
            return Err(LoweringError::UnsupportedStructuralReference(root));
        }
    }
    let (selected, shape, offset, indices) = runtime_borrowed_projection(
        root,
        path,
        requested,
        expected,
        declarations,
        &mut shapes,
        &mut active,
    )?;
    runs.extend(indices);
    Ok((selected, shape, offset, runs))
}

/// Resolve a primitive address in path order. A reference declaration supplies
/// pointer geometry, not permission: the admitted operation must independently
/// establish live authority for every crossing. Origins never choose an address.
pub(super) fn primitive_leaf_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<PrimitiveProjection> {
    primitive_access_projection(root, path, declarations, contiguous_primitive_projection)
}

fn primitive_access_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    endpoint: impl Fn(
        StructuralTypeId,
        &[StructuralPathSegment],
        &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)>,
) -> Option<PrimitiveProjection> {
    let regions = reference_projection_regions(root, path, declarations)?;
    let (scalar, byte_offset, indices) = endpoint(regions.root, regions.path, declarations)?;
    Some(PrimitiveProjection {
        scalar,
        pointer_slots: regions.pointer_slots,
        leaf: StorageProjection {
            byte_offset,
            indices,
        },
    })
}

/// Separate contiguous address regions before selecting an operation's leaf.
/// Primitive and byte-field observations share each ordered pointer crossing;
/// endpoint geometry and live permission remain separate judgments.
pub(super) struct ReferenceProjectionRegions<'path> {
    pub(super) root: StructuralTypeId,
    pub(super) path: &'path [StructuralPathSegment],
    pub(super) pointer_slots: Vec<StorageProjection>,
    pub(super) readable: bool,
}

pub(super) fn reference_projection_regions<'path>(
    mut root: StructuralTypeId,
    path: &'path [StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<ReferenceProjectionRegions<'path>> {
    let mut pointer_slots = Vec::new();
    let mut readable = true;
    let mut segment_start = 0;
    for (position, segment) in path.iter().enumerate() {
        if !matches!(
            segment,
            StructuralPathSegment::Referent
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
        ) {
            continue;
        }
        let selected =
            runtime_descriptor_projection(root, &path[segment_start..position], declarations).ok();
        let Some((carrier, shape, byte_offset, indices)) = selected else {
            // Byte primitive endpoint geometry owns its noncanonical field
            // leaves. A reference crossing always requires a declared carrier.
            if *segment == StructuralPathSegment::Referent {
                return None;
            }
            continue;
        };
        let (next_root, next_start) = match (segment, &declarations.get(&carrier)?.shape) {
            (
                StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, access },
            ) => {
                readable &= matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                );
                if !matches!(shape.byte_size, 8 | 16) || shape.alignment != 8 {
                    return None;
                }
                (*referent, position + 1)
            }
            (
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { .. },
            ) => {
                // The selector belongs to the backing region reached by this
                // load; it does not index the descriptor carrier itself.
                if shape.byte_size != 16 || shape.alignment != 8 {
                    return None;
                }
                (carrier, position)
            }
            (
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. },
                borrowed,
            ) if borrowed.is_borrowed_byte_carrier() => (carrier, position),
            (StructuralPathSegment::Referent, _) => return None,
            _ => continue,
        };
        pointer_slots.push(StorageProjection {
            byte_offset,
            indices,
        });
        root = next_root;
        segment_start = next_start;
    }
    Some(ReferenceProjectionRegions {
        root,
        path: &path[segment_start..],
        pointer_slots,
        readable,
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
    if let Some((selector, prefix)) = path.split_last()
        && matches!(
            selector,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
        )
        && let Ok((selected, _, owner_offset, mut indices)) = runtime_projection(
            root,
            prefix,
            declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        && let StructuralTypeShape::ByteSequence(carrier) = declarations.get(&selected)?.shape
    {
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
        // First select the owner within this contiguous region, then its byte.
        // An array/record prefix retains its own offset and selectors. Pointer
        // regions already load borrowed backing, whose payload offset is zero.
        let payload_offset = owner_offset.checked_add(payload_offset)?;
        return match selector {
            StructuralPathSegment::FixedIndex(index) => Some((
                scalar,
                payload_offset.checked_add(u32::try_from(*index).ok()?)?,
                indices,
            )),
            StructuralPathSegment::RuntimeIndex { index, .. } => {
                indices.push((*index, 1));
                Some((scalar, payload_offset, indices))
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
        if let Some(fields) = declarations.get(&carrier)?.shape.direct_fields() {
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
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

/// Admit the complete primitive read before resolving its ordered pointer
/// regions. A runtime selector may precede a reference crossing; checking only
/// the final region would lose that selector and reject its scalar field.
/// Each region reconstructs geometry from its own declarations, while the
/// complete path retains the operation's distinct read endpoint rules.
#[allow(clippy::type_complexity)]
pub(super) fn primitive_read_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<PrimitiveProjection> {
    let scalar = terminal_semantics::primitive_read_projection_type(
        declarations.values().copied(),
        root,
        path,
    )?;
    let projection = primitive_access_projection(
        root,
        path,
        declarations,
        contiguous_primitive_read_projection,
    )?;
    (projection.scalar == scalar).then_some(projection)
}

#[allow(clippy::type_complexity)]
fn contiguous_primitive_read_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<(ScalarType, u32, Vec<(semantic_vocabulary::ValueId, u32)>)> {
    if let Some(leaf) = contiguous_primitive_projection(root, path, declarations) {
        return Some(leaf);
    }
    let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
        return None;
    };
    let field =
        terminal_semantics::runtime_structural_field(declarations.values().copied(), root, path)?;
    let (prefix, namespace) = match carrier.split_last() {
        Some((StructuralPathSegment::Case(_), prefix)) => (prefix, &carrier[carrier.len() - 1..]),
        _ => (carrier, &[][..]),
    };
    let (carrier_type, _, carrier_offset, runs) = runtime_projection(
        root,
        prefix,
        declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .ok()?;
    // Eligibility was checked on the complete read. This suffix supplies only
    // the scalar's storage layout, never store authority or a canonical fact.
    let scalar = field.field_type.scalar_type()?;
    let (field_offset, _) = if namespace.is_empty() {
        direct_scalar_field_geometry(carrier_type, field.id, scalar, declarations).ok()?
    } else {
        static_field_geometry(carrier_type, namespace, field.id, scalar, declarations).ok()?
    };
    Some((scalar, carrier_offset.checked_add(field_offset)?, runs))
}

mod scalar_fields;
pub(super) use scalar_fields::{
    direct_scalar_field_geometry, direct_scalar_field_offset, static_field_geometry,
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
            StructuralTypeShape::BindingCarrier => Ok(ValueShape::integer(0, 1)),
            // All supported native ABIs use 64-bit addresses. The carrier
            // stores the referent's address, never its contents; custody is
            // checked separately. Nested reference leaves use the same layout
            // as whole results, so enclosing record offsets remain coherent.
            StructuralTypeShape::Reference { referent, .. } => Ok(ValueShape::integer(
                if scalar_storage_type(*referent, declarations).is_some() {
                    16
                } else {
                    8
                },
                8,
            )),
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
            StructuralTypeShape::Record { fields, storage } => {
                if matches!(
                    storage,
                    terminal_psi::StructuralRecordStorage::TargetLayout(_)
                ) {
                    return Err(LoweringError::UnclosedTargetLayout(structural_type));
                }
                if let terminal_psi::StructuralRecordStorage::Authored(storage)
                | terminal_psi::StructuralRecordStorage::PlacedView(storage)
                | terminal_psi::StructuralRecordStorage::PlacedAccessor(storage) = storage
                {
                    return Ok(ValueShape::integer(
                        u32::try_from(storage.byte_size)
                            .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
                        u16::try_from(storage.byte_alignment)
                            .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
                    ));
                }
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

/// Internal scalar loans carry a descriptor; aggregate loans remain thin.
pub(super) fn internal_parameter_shape(
    structural_type: StructuralTypeId,
    referent: ValueShape,
    access: StructuralAccess,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> ValueShape {
    if access != StructuralAccess::Owned
        && scalar_storage_type(structural_type, declarations).is_some()
    {
        ValueShape::borrowed_scalar_reference()
    } else {
        structural_parameter_shape(referent, access)
    }
}

pub(super) fn scalar_storage_type(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<ScalarType> {
    let StructuralTypeShape::PrimitiveScalar(scalar) = declarations.get(&structural_type)?.shape
    else {
        return None;
    };
    runtime_abi::ScalarStorageCodec::from_scalar(scalar).map(|_| scalar)
}

pub(super) use crate::validation::structural_shapes::parameter_shape as structural_parameter_shape;

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

fn structural_field_storage_shape(
    field: &terminal_psi::StructuralFieldDeclaration,
    owner: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, LoweringError> {
    if matches!(
        field.storage.as_ref(),
        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
    ) {
        return Err(LoweringError::UnknownStructuralType(owner));
    }
    if let Some(stored) = field
        .storage
        .as_ref()
        .and_then(|storage| storage.stored_scalar())
    {
        let codec = runtime_abi::ScalarStorageCodec::from_scalar(stored)
            .ok_or(LoweringError::UnknownStructuralType(owner))?;
        return Ok(ValueShape::integer(u32::from(codec.byte_size()), 1));
    }
    structural_field_shape(&field.field_type, owner, declarations, cache, active)
}

fn structural_field_offset(
    field: &terminal_psi::StructuralFieldDeclaration,
    conventional_offset: u32,
    alignment: u16,
    owner: StructuralTypeId,
) -> Result<u32, LoweringError> {
    match field.storage.as_ref() {
        Some(storage) => storage
            .byte_offset()
            .and_then(|offset| u32::try_from(offset).ok())
            .ok_or(LoweringError::UnknownStructuralType(owner)),
        None => checked_align_up_u32(conventional_offset, u32::from(alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(owner)),
    }
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
    projection_path(
        structural_type,
        path,
        declarations,
        cache,
        active,
        StructuralFieldType::canonical_leaf_type,
    )
}

fn field_geometry<'types>(
    structural_type: StructuralTypeId,
    segment: &StructuralPathSegment,
    declarations: &BTreeMap<StructuralTypeId, &'types StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(&'types StructuralFieldType, ValueShape, u32, Option<u64>), LoweringError> {
    let root_type = structural_type;
    let StructuralPathSegment::Field(identity) = segment else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    let Some(fields) = declaration.shape.direct_fields() else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    // Mixed common members share Field identity with record members but
    // sit after the tag. Use their actual conventional-sum placements,
    // never a record-relative accumulation that would read tag bytes.
    let common_layout = if matches!(declaration.shape, StructuralTypeShape::Mixed { .. }) {
        Some(structural_sum_layout(
            structural_type,
            declarations,
            cache,
            active,
        )?)
    } else {
        None
    };
    let mut local_offset = 0_u32;
    for (field_position, field) in fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .enumerate()
    {
        if matches!(field.field_type, StructuralFieldType::Erased { .. }) {
            continue;
        }
        if field.storage.is_some() && field.identity != *identity {
            continue;
        }
        let field_shape =
            structural_field_storage_shape(field, structural_type, declarations, cache, active)?;
        local_offset = match &common_layout {
            Some(layout) => {
                layout
                    .common_fields
                    .get(field_position)
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?
                    .byte_offset
            }
            None => structural_field_offset(field, local_offset, field_shape.alignment, root_type)?,
        };
        if field.identity == *identity {
            let repeated_stride = field
                .storage
                .as_ref()
                .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride);
            return Ok((
                &field.field_type,
                field_shape,
                local_offset,
                repeated_stride,
            ));
        }
        local_offset = local_offset
            .checked_add(field_shape.byte_size)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
    }
    Err(LoweringError::UnknownStructuralType(root_type))
}

/// A byte-sequence field is a leaf carrier, not a structural type: the path's
/// last segment names a field carrying `ByteSequence` directly. Resolve
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
    if !matches!(path.last(), Some(StructuralPathSegment::Field(_))) {
        return Ok(None);
    }
    let (selected, _, offset, indices) =
        runtime_storage_projection(structural_type, path, declarations, cache, active)?;
    if !indices.is_empty() {
        return Err(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ));
    }
    Ok(match selected {
        ProjectedStorage::Field(field) => field
            .byte_sequence_carrier()
            .map(|carrier| (offset, carrier)),
        ProjectedStorage::Declaration(_) => None,
    })
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
                let case_position =
                    terminal_psi::structural_case_position(cases, *identity).ok_or_else(invalid)?;
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
                let Some(next) = declaration.field_type.as_structural() else {
                    return Err(invalid());
                };
                let layout = structural_sum_layout(current, declarations, cache, active)?;
                (
                    next,
                    layout.cases[case_position].fields[field_position].byte_offset,
                )
            }
            Segment::Field(identity) => {
                let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = shape else {
                    return Err(invalid());
                };
                let (field_position, field) = fields
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                    .enumerate()
                    .find(|(_, field)| field.id == *identity)
                    .ok_or_else(invalid)?;
                let Some(next) = field.field_type.as_structural() else {
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
        StructuralFieldType::canonical_leaf_type,
    )
}

fn projection_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Result<(StructuralTypeId, ValueShape, u32), LoweringError> {
    let (selected, shape, offset, indices) = storage_projection_path(
        structural_type,
        path,
        declarations,
        cache,
        active,
        leaf_type,
    )?;
    if !indices.is_empty() {
        return Err(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ));
    }
    Ok((selected, shape, offset))
}

fn storage_projection_path(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Result<RuntimeProjection, LoweringError> {
    let (selected, shape, offset, indices) =
        runtime_storage_projection(structural_type, path, declarations, cache, active)?;
    Ok((
        selected.declaration(structural_type, declarations, leaf_type)?,
        shape,
        offset,
        indices,
    ))
}

/// An inline field has its own carrier contract, not a standalone declaration
/// chosen by layout equality. Geometry consumers keep this occurrence; consumers
/// requiring a type coordinate explicitly resolve their leaf presentation.
#[derive(Clone, Copy)]
pub(super) enum ProjectedStorage<'types> {
    Declaration(StructuralTypeId),
    Field(&'types StructuralFieldType),
}

impl ProjectedStorage<'_> {
    pub(super) fn declaration(
        self,
        root: StructuralTypeId,
        declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
    ) -> Result<StructuralTypeId, LoweringError> {
        let leaf = match self {
            Self::Declaration(identity) => return Ok(identity),
            Self::Field(StructuralFieldType::Structural(identity)) => return Ok(*identity),
            Self::Field(leaf) => leaf,
        };
        let declaration = match (leaf_type(leaf), leaf.byte_sequence_carrier()) {
            (Some(StructuralFieldType::Structural(identity)), _) => {
                declarations.get(&identity).copied()
            }
            (Some(contract), _) => contract.leaf_declaration(declarations.values().copied()),
            (None, Some(carrier)) => terminal_semantics::byte_carrier_field_declaration(
                declarations.values().copied(),
                carrier,
            ),
            (None, None) => None,
        };
        declaration
            .map(|declaration| declaration.id)
            .ok_or(LoweringError::UnknownStructuralType(root))
    }

    pub(super) fn shape<'types>(
        self,
        declarations: &BTreeMap<StructuralTypeId, &'types StructuralTypeDeclaration>,
    ) -> Option<std::borrow::Cow<'types, StructuralTypeShape>> {
        match self {
            Self::Declaration(identity) => declarations
                .get(&identity)
                .map(|row| std::borrow::Cow::Borrowed(&row.shape)),
            Self::Field(StructuralFieldType::Structural(identity)) => declarations
                .get(identity)
                .map(|row| std::borrow::Cow::Borrowed(&row.shape)),
            Self::Field(field) => field
                .byte_sequence_carrier()
                .map(|carrier| std::borrow::Cow::Owned(StructuralTypeShape::ByteSequence(carrier))),
        }
    }
}

type RuntimeStorageProjection<'types> = (
    ProjectedStorage<'types>,
    ValueShape,
    u32,
    Vec<(semantic_vocabulary::ValueId, u32)>,
);

// One ordered walk keeps an authored array stride across its selecting field.
// Static and runtime indices consume that same storage occurrence. Final inline
// leaves need no global type lookup: equal byte capacities can have distinct
// qualified identities, and unrelated declarations cannot change field geometry.
pub(super) fn runtime_storage_projection<'types>(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &'types StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<RuntimeStorageProjection<'types>, LoweringError> {
    let root_type = structural_type;
    let mut selected_storage = ProjectedStorage::Declaration(structural_type);
    let mut total_offset = 0_u32;
    let mut selected_shape = None;
    let mut indices = Vec::new();
    let mut repeated_stride = None;
    let mut segments = path.iter();
    while let Some(segment) = segments.next() {
        structural_type = match selected_storage {
            ProjectedStorage::Declaration(identity) => identity,
            ProjectedStorage::Field(StructuralFieldType::Structural(identity)) => *identity,
            ProjectedStorage::Field(_) => {
                return Err(LoweringError::UnknownStructuralType(root_type));
            }
        };
        let (selected_type, shape, local_offset) = match segment {
            StructuralPathSegment::AllElements => {
                return Err(LoweringError::UnknownStructuralType(structural_type));
            }
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
                let (field_position, field) =
                    terminal_psi::live_field_entry(&cases[case_position].fields, *identity)
                        .ok_or_else(invalid)?;
                repeated_stride = field
                    .storage
                    .as_ref()
                    .and_then(semantic_vocabulary::AuthoredFieldStorage::element_stride);
                let layout = structural_sum_layout(structural_type, declarations, cache, active)?;
                (
                    ProjectedStorage::Field(&field.field_type),
                    structural_field_storage_shape(
                        field,
                        structural_type,
                        declarations,
                        cache,
                        active,
                    )?,
                    layout.cases[case_position].fields[field_position].byte_offset,
                )
            }
            // Returned-origin interpretations do not select an executable
            // address. Actual reference formation owns the emitted pointer;
            // an origin recipe must not replace that physical operand path.
            StructuralPathSegment::RepresentationView { .. }
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. } => {
                return Err(LoweringError::UnsupportedStructuralReference(
                    structural_type,
                ));
            }
            StructuralPathSegment::Field(_) => {
                let (selected, shape, offset, stride) =
                    field_geometry(structural_type, segment, declarations, cache, active)?;
                repeated_stride = stride;
                (ProjectedStorage::Field(selected), shape, offset)
            }
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
                let declaration = declarations
                    .get(&structural_type)
                    .copied()
                    .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, length } => {
                        if matches!(segment, StructuralPathSegment::FixedIndex(index) if index >= length)
                        {
                            return Err(LoweringError::UnknownStructuralType(structural_type));
                        }
                        *element
                    }
                    // A view's storage is element-contiguous like the
                    // array's; its bound lives in the verifier's extent
                    // obligation rather than a literal type length.
                    StructuralTypeShape::ElementView { element, .. } => *element,
                    _ => return Err(LoweringError::UnknownStructuralType(structural_type)),
                };
                let shape = structural_shape(element, declarations, cache, active)?;
                let stride = match repeated_stride.take() {
                    Some(stride) => u32::try_from(stride)
                        .map_err(|_| LoweringError::StructuralTypeTooLarge(root_type))?,
                    None => checked_align_up_u32(shape.byte_size, u32::from(shape.alignment))
                        .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?,
                };
                let offset = match segment {
                    StructuralPathSegment::FixedIndex(index) => u64::from(stride)
                        .checked_mul(*index)
                        .and_then(|offset| u32::try_from(offset).ok())
                        .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?,
                    StructuralPathSegment::RuntimeIndex { index, .. } => {
                        indices.push((*index, stride));
                        0
                    }
                    _ => return Err(LoweringError::UnknownStructuralType(structural_type)),
                };
                (ProjectedStorage::Declaration(element), shape, offset)
            }
        };
        total_offset = total_offset
            .checked_add(local_offset)
            .ok_or(LoweringError::StructuralTypeTooLarge(root_type))?;
        selected_storage = selected_type;
        selected_shape = Some(shape);
    }
    // The empty path is the identity projection, not a missing declaration.
    // Resolve its root through the same layout validation as every child.
    let shape = match selected_shape {
        Some(shape) => shape,
        None => structural_shape(structural_type, declarations, cache, active)?,
    };
    Ok((selected_storage, shape, total_offset, indices))
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
/// Descriptor geometry keeps an inline byte field in its original storage.
/// Its carrier declaration is a type coordinate, not a copied descriptor.
pub(super) fn runtime_descriptor_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<RuntimeProjection, LoweringError> {
    runtime_projection_with_leaf_type(
        root,
        path,
        declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
        |field| {
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
        },
    )
}

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
    runtime_projection_with_leaf_type(
        structural_type,
        path,
        declarations,
        cache,
        active,
        StructuralFieldType::canonical_leaf_type,
    )
}

/// A snapshot may copy a declared byte field without making that field an
/// ordinary borrow or call projection. Qualification custody stays with the
/// copy operation's independent source replay.
pub(super) fn runtime_leaf_copy_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    expected: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<RuntimeProjection, LoweringError> {
    // A snapshot follows the same ordered descriptor crossings as other
    // observations. Flatten only the selector roster for operand custody;
    // selection reconstructs each region from the retained path.
    let expected = declarations
        .get(&expected)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(expected))?;
    let regions = reference_projection_regions(structural_type, path, declarations)
        .filter(|regions| regions.readable)
        .ok_or(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ))?;
    let (selected, shape, byte_offset, leaf_indices) = runtime_projection_with_leaf_type(
        regions.root,
        regions.path,
        declarations,
        cache,
        active,
        |field| field.leaf_copy_type(Some(expected)),
    )?;
    let indices = regions
        .pointer_slots
        .into_iter()
        .flat_map(|slot| slot.indices)
        .chain(leaf_indices)
        .collect();
    Ok((selected, shape, byte_offset, indices))
}

/// Borrowed call geometry retains the original field's physical storage.
/// Scalar loans present their carrier without transferring the field invariant:
/// Psi checked its preservation on writable handback. This layout query keeps
/// the actual field address; it grants no store or predicate evidence.
pub(super) fn runtime_borrowed_projection(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    expected: Option<StructuralTypeId>,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<RuntimeProjection, LoweringError> {
    let regions = reference_projection_regions(structural_type, path, declarations).ok_or(
        LoweringError::UnsupportedStructuralReference(structural_type),
    )?;
    let expected = expected.and_then(|expected| declarations.get(&expected).copied());
    let (selected, shape, offset, leaf_indices) = runtime_projection_with_leaf_type(
        regions.root,
        regions.path,
        declarations,
        cache,
        active,
        |field| field.borrowed_presentation_type(access, expected),
    )?;
    let indices = regions
        .pointer_slots
        .into_iter()
        .flat_map(|slot| slot.indices)
        .chain(leaf_indices)
        .collect();
    Ok((selected, shape, offset, indices))
}

fn runtime_projection_with_leaf_type(
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaf_type: impl Fn(&StructuralFieldType) -> Option<StructuralFieldType>,
) -> Result<RuntimeProjection, LoweringError> {
    // Byte carriers have primitive leaves rather than an element type handle.
    // Resolve the same endpoint used by primitive reads/stores, then rejoin its
    // canonical scalar declaration for borrowed-call signature transport.
    if !path.is_empty()
        && matches!(
            declarations.get(&structural_type).map(|row| &row.shape),
            Some(StructuralTypeShape::ByteSequence(_))
        )
    {
        // This query describes one contiguous backing region. The enclosing
        // reference/view walk owns pointer loads; asking that full walk for
        // this endpoint would reject its legitimate descriptor crossing (and
        // couple ordinary and foreign byte-element loans to snapshot geometry).
        let (_, byte_offset, indices) =
            contiguous_primitive_projection(structural_type, path, declarations).ok_or(
                LoweringError::UnsupportedStructuralReference(structural_type),
            )?;
        let endpoint = terminal_semantics::primitive_projection_declaration(
            declarations.values().copied(),
            structural_type,
            path,
        )
        .ok_or(LoweringError::UnsupportedStructuralReference(
            structural_type,
        ))?;
        let shape = structural_shape(endpoint, declarations, cache, active)?;
        return Ok((endpoint, shape, byte_offset, indices));
    }
    storage_projection_path(
        structural_type,
        path,
        declarations,
        cache,
        active,
        leaf_type,
    )
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
mod placed_geometry_tests {
    use super::*;
    use semantic_vocabulary::{AuthoredFieldStorage, AuthoredRecordStorage, StructuralFieldId};
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralRecordStorage, TerminalIdentityIndex,
    };

    #[test]
    fn placed_container_geometry_preserves_offset_without_scalar_authority() {
        let root = StructuralTypeId::new(1).unwrap();
        let accessor = StructuralTypeId::new(2).unwrap();
        let field = StructuralFieldId::new(1).unwrap();
        let accessor_declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            id: accessor,
            identity: TerminalIdentityIndex(2),
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                fields: vec![],
                storage: StructuralRecordStorage::PlacedAccessor(AuthoredRecordStorage {
                    byte_size: 4,
                    byte_alignment: 1,
                }),
            },
        };
        let root_declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            id: root,
            identity: TerminalIdentityIndex(1),
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: TerminalIdentityIndex(3),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(accessor),
                    storage: Some(AuthoredFieldStorage::Direct { byte_offset: 4 }),
                }],
                storage: StructuralRecordStorage::PlacedView(AuthoredRecordStorage {
                    byte_size: 8,
                    byte_alignment: 4,
                }),
            },
        };
        let declarations =
            BTreeMap::from([(root, &root_declaration), (accessor, &accessor_declaration)]);
        let (selected, shape, offset, indices) = runtime_projection(
            root,
            &[StructuralPathSegment::Field(TerminalIdentityIndex(3))],
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(selected, accessor);
        assert_eq!(shape, ValueShape::integer(4, 1));
        assert_eq!(offset, 4);
        assert!(indices.is_empty());
        assert!(
            primitive_leaf_projection(
                root,
                &[StructuralPathSegment::Field(TerminalIdentityIndex(3))],
                &declarations
            )
            .is_none()
        );
        assert!(!terminal_semantics::zeroed_structural_shape(
            declarations.values().copied(),
            root
        ));
        assert!(!terminal_semantics::zeroed_structural_shape(
            declarations.values().copied(),
            accessor
        ));
    }
}
