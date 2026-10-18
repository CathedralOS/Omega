//! Producer-independent referent shape reconstruction for structural ABI validation.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{
    CallingPolicy, ConventionalSumLayout, SystemVEightbyteClass, ValueClass, ValueShape,
};
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidStructuralShape;

/// Index declarations by id, rejecting a duplicate-id roster.
fn index_declarations(
    declarations: &[StructuralTypeDeclaration],
) -> Result<BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>, InvalidStructuralShape> {
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    Ok(indexed)
}

pub(crate) fn parameter_shape(referent: ValueShape, access: StructuralAccess) -> ValueShape {
    match access {
        StructuralAccess::Owned => referent,
        StructuralAccess::SharedBorrow
        | StructuralAccess::MutableBorrow
        | StructuralAccess::WriteOnlyBorrow => {
            ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
        }
    }
}

/// The by-value boundary shape one structural type carries under the calling
/// policy: the provider-side materialization classifies records and fixed
/// arrays by their scalar leaves — homogeneous-float members under AAPCS and
/// SysV, per-eightbyte integer/SSE classes under SysV, one integer view under
/// Microsoft x64. Scalar and descriptor carriers keep their semantic shape;
/// carriers with no by-value boundary encoding refuse, matching the upstream
/// materialization that produced the plan being replayed.
pub fn classified_boundary_shape(
    structural_type: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
    policy: CallingPolicy,
) -> Result<ValueShape, InvalidStructuralShape> {
    let indexed = index_declarations(declarations)?;
    let declaration = indexed
        .get(&structural_type)
        .copied()
        .ok_or(InvalidStructuralShape)?;
    let referent = shape(
        structural_type,
        &indexed,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let class = match &declaration.shape {
        terminal_psi::record_or_fixed_array_shape_pattern!() => {
            let mut leaves = Vec::new();
            collect_scalar_leaves(
                structural_type,
                0,
                &indexed,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                &mut leaves,
                0,
            )?;
            classify_scalar_leaves(&leaves, referent, policy)?
        }
        StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralTypeShape::ElementView { .. } => referent.class,
        _ => return Err(InvalidStructuralShape),
    };
    Ok(ValueShape {
        class,
        byte_size: referent.byte_size,
        alignment: referent.alignment,
    })
}

/// Every scalar leaf one structural carrier contributes to aggregate
/// classification: `(byte_offset, byte_size, is_float)` rows in declared
/// storage order, matching the leaves the boundary materialization collects
/// from the typed signature graph. Descriptor views and references are single
/// integer leaves; inline byte storage contributes one leaf per byte.
#[allow(clippy::too_many_arguments)]
fn collect_scalar_leaves(
    structural_type: StructuralTypeId,
    base_offset: u32,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaves: &mut Vec<(u32, u32, bool)>,
    depth: usize,
) -> Result<(), InvalidStructuralShape> {
    if depth > 32 {
        return Err(InvalidStructuralShape);
    }
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(InvalidStructuralShape)?;
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            let leaf = scalar_shape(*scalar);
            leaves.push((
                base_offset,
                leaf.byte_size,
                matches!(scalar, ScalarType::IeeeFloat(_)),
            ));
        }
        StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralTypeShape::ElementView { .. } => {
            let referent = shape(structural_type, declarations, cache, active)?;
            leaves.push((base_offset, referent.byte_size, false));
        }
        StructuralTypeShape::Record { fields, .. } => {
            let mut local_offset = 0_u32;
            for field in fields.iter().filter(|field| {
                !field.relevance.is_erased()
                    && !matches!(field.field_type, StructuralFieldType::Erased { .. })
            }) {
                let field_shape = field_shape(&field.field_type, declarations, cache, active)?;
                local_offset = align(local_offset, u32::from(field_shape.alignment))?;
                collect_field_leaves(
                    &field.field_type,
                    base_offset
                        .checked_add(local_offset)
                        .ok_or(InvalidStructuralShape)?,
                    declarations,
                    cache,
                    active,
                    leaves,
                    depth + 1,
                )?;
                local_offset = local_offset
                    .checked_add(field_shape.byte_size)
                    .ok_or(InvalidStructuralShape)?;
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let element_shape = shape(*element, declarations, cache, active)?;
            for index in 0..*length {
                let offset = u64::from(element_shape.byte_size)
                    .checked_mul(index)
                    .and_then(|element_offset| u64::from(base_offset).checked_add(element_offset))
                    .and_then(|total| u32::try_from(total).ok())
                    .ok_or(InvalidStructuralShape)?;
                collect_scalar_leaves(
                    *element,
                    offset,
                    declarations,
                    cache,
                    active,
                    leaves,
                    depth + 1,
                )?;
            }
        }
        _ => return Err(InvalidStructuralShape),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_field_leaves(
    field: &StructuralFieldType,
    base_offset: u32,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    leaves: &mut Vec<(u32, u32, bool)>,
    depth: usize,
) -> Result<(), InvalidStructuralShape> {
    match field {
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
        | StructuralFieldType::IeeeFloat(_) => {
            let leaf = field_shape(field, declarations, cache, active)?;
            leaves.push((base_offset, leaf.byte_size, true));
        }
        StructuralFieldType::Scalar(_)
        | StructuralFieldType::Atomic(_)
        | StructuralFieldType::RefinedScalar(_)
        | StructuralFieldType::BoundedInteger(_) => {
            let leaf = field_shape(field, declarations, cache, active)?;
            leaves.push((base_offset, leaf.byte_size, false));
        }
        StructuralFieldType::ByteSequence(terminal_psi::byte_sequence_view_carrier_pattern!())
        | StructuralFieldType::PredicateByteSequence {
            carrier: terminal_psi::byte_sequence_view_carrier_pattern!(),
            ..
        } => {
            let leaf = field_shape(field, declarations, cache, active)?;
            leaves.push((base_offset, leaf.byte_size, false));
        }
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
        | StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BoundedOwned { .. },
            ..
        } => {
            let leaf = field_shape(field, declarations, cache, active)?;
            for index in 0..leaf.byte_size {
                leaves.push((
                    base_offset
                        .checked_add(index)
                        .ok_or(InvalidStructuralShape)?,
                    1,
                    false,
                ));
            }
        }
        StructuralFieldType::Structural(nested) => collect_scalar_leaves(
            *nested,
            base_offset,
            declarations,
            cache,
            active,
            leaves,
            depth + 1,
        )?,
        StructuralFieldType::Erased { .. } => return Err(InvalidStructuralShape),
    }
    Ok(())
}

/// The ABI class the collected scalar leaves determine under the calling
/// policy: homogeneous-float aggregates, SysV per-eightbyte classes, or one
/// integer view — identical to the classification the boundary
/// materialization applied when it evaluated the plan under replay.
fn classify_scalar_leaves(
    leaves: &[(u32, u32, bool)],
    referent: ValueShape,
    policy: CallingPolicy,
) -> Result<ValueClass, InvalidStructuralShape> {
    if policy == CallingPolicy::MicrosoftX64 {
        return Ok(ValueClass::Integer);
    }
    // AAPCS64's four-member rule is not SysV's two-eightbyte limit. Larger
    // SysV float aggregates take the memory-placement class below.
    if let Some(members) = homogeneous_float_leaves(leaves, referent) {
        if policy == CallingPolicy::Aapcs64
            || (policy == CallingPolicy::SystemVAMD64 && members > 1 && referent.byte_size <= 16)
        {
            return Ok(ValueClass::HomogeneousFloatAggregate { members });
        }
        if policy == CallingPolicy::SystemVAMD64 && members == 1 {
            return Ok(ValueClass::Float);
        }
    }
    if policy != CallingPolicy::SystemVAMD64 || referent.byte_size > 16 {
        return Ok(ValueClass::Integer);
    }
    let mut classes = [None, None];
    for &(offset, byte_size, is_float) in leaves {
        let eightbyte = usize::try_from(offset / 8).map_err(|_| InvalidStructuralShape)?;
        let last = offset
            .checked_add(byte_size)
            .and_then(|end| end.checked_sub(1))
            .ok_or(InvalidStructuralShape)?;
        if eightbyte > 1
            || usize::try_from(last / 8).map_err(|_| InvalidStructuralShape)? != eightbyte
        {
            return Err(InvalidStructuralShape);
        }
        classes[eightbyte] = Some(match classes[eightbyte] {
            Some(existing_is_sse) => existing_is_sse && is_float,
            None => is_float,
        });
    }
    let first = classes[0].ok_or(InvalidStructuralShape)?;
    let second = if referent.byte_size > 8 {
        classes[1].ok_or(InvalidStructuralShape)?
    } else {
        false
    };
    if !first && !second {
        return Ok(ValueClass::Integer);
    }
    if referent.byte_size <= 8 {
        return Err(InvalidStructuralShape);
    }
    Ok(ValueClass::SystemVAggregate {
        first: if first {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
        second: if second {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
    })
}

/// A homogeneous float aggregate is a dense run of one to four equal IEEE
/// members starting at offset zero, covering the referent exactly.
fn homogeneous_float_leaves(leaves: &[(u32, u32, bool)], referent: ValueShape) -> Option<u8> {
    let &(0, member_size, true) = leaves.first()? else {
        return None;
    };
    let members = u8::try_from(leaves.len()).ok()?;
    ((1..=4).contains(&members)
        && matches!(member_size, 4 | 8)
        && leaves
            .iter()
            .enumerate()
            .all(|(index, &(offset, size, is_float))| {
                is_float && size == member_size && offset == index as u32 * member_size
            })
        && referent.byte_size == member_size * u32::from(members)
        && referent.alignment == u16::try_from(member_size).unwrap_or(0))
    .then_some(members)
}

fn shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, InvalidStructuralShape> {
    if let Some(shape) = cache.get(&structural_type) {
        return Ok(*shape);
    }
    if !active.insert(structural_type) {
        return Err(InvalidStructuralShape);
    }
    let declaration = declarations
        .get(&structural_type)
        .ok_or(InvalidStructuralShape)?;
    let result = match &declaration.shape {
        StructuralTypeShape::BindingCarrier => ValueShape::integer(0, 1),
        // Independently reconstruct the address payload, including nested
        // record leaves. Pointer bits carry no ownership authority.
        StructuralTypeShape::Reference { referent, .. } => ValueShape::integer(
            if declarations.get(referent).is_some_and(|declaration| matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(scalar) if runtime_abi::ScalarStorageCodec::from_scalar(scalar).is_some())) { 16 } else { 8 }, 8),
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_shape(*scalar),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralTypeShape::ElementView { .. } => ValueShape::integer(16, 8),
        // The declaration supplies the fixed extent; only its backing pointer
        // occupies descriptor storage. Reconstruct this without producer rows.
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView { .. }) => {
            ValueShape::integer(8, 8)
        }
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => {
            let bytes = capacity.checked_add(8).ok_or(InvalidStructuralShape)?;
            ValueShape::integer(u32::try_from(bytes).map_err(|_| InvalidStructuralShape)?, 8)
        }
        StructuralTypeShape::Record { fields, storage } => {
            if matches!(storage, terminal_psi::StructuralRecordStorage::TargetLayout(_)) {
                return Err(InvalidStructuralShape);
            }
            if let terminal_psi::StructuralRecordStorage::Authored(storage) | terminal_psi::StructuralRecordStorage::PlacedView(storage) | terminal_psi::StructuralRecordStorage::PlacedAccessor(storage) = storage {
                active.remove(&structural_type);
                return Ok(ValueShape::integer(u32::try_from(storage.byte_size).map_err(|_| InvalidStructuralShape)?, u16::try_from(storage.byte_alignment).map_err(|_| InvalidStructuralShape)?));
            }
            let mut byte_size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields.iter().filter(|field| {
                !field.relevance.is_erased()
                    && !matches!(field.field_type, StructuralFieldType::Erased { .. })
            }) {
                let field_shape = field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                byte_size = align(byte_size, u32::from(field_shape.alignment))?;
                byte_size = byte_size
                    .checked_add(field_shape.byte_size)
                    .ok_or(InvalidStructuralShape)?;
            }
            byte_size = align(byte_size, u32::from(alignment))?;
            ValueShape::integer(byte_size, alignment)
        }
        StructuralTypeShape::FixedArray { length: 0, .. } => {
            // Zero-byte calling plans retain a canonical shape. Reconstruct
            // the complete element declaration even though no element bytes
            // are transported; unknown and recursive shells still reject.
            validate_empty_array_element(structural_type, declarations, cache, active)?;
            ValueShape::integer(0, 1)
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let element = shape(*element, declarations, cache, active)?;
            let stride = align(element.byte_size, u32::from(element.alignment))?;
            let bytes = u64::from(stride)
                .checked_mul(*length)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(InvalidStructuralShape)?;
            ValueShape::integer(bytes, element.alignment)
        }
        StructuralTypeShape::Sum { cases } if !cases.is_empty() => {
            conventional_sum_shape(&[], cases, declarations, cache, active)?
        }
        StructuralTypeShape::Mixed { fields, cases } if !cases.is_empty() => {
            conventional_sum_shape(fields, cases, declarations, cache, active)?
        }
        _ => return Err(InvalidStructuralShape),
    };
    active.remove(&structural_type);
    cache.insert(structural_type, result);
    Ok(result)
}

fn validate_empty_array_element(
    root: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<(), InvalidStructuralShape> {
    let mut current = root;
    for _ in 0..declarations.len() {
        match declarations
            .get(&current)
            .ok_or(InvalidStructuralShape)?
            .shape
        {
            StructuralTypeShape::FixedArray { element, .. } => current = element,
            StructuralTypeShape::PrimitiveScalar(scalar) => {
                return match scalar {
                    ScalarType::Boolean | ScalarType::IeeeFloat(_) => Ok(()),
                    ScalarType::Integer(integer)
                        if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                            && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
                    {
                        Ok(())
                    }
                    _ => Err(InvalidStructuralShape),
                };
            }
            // The array chain's extents contribute no payload under the
            // empty root. A nominal element still has its own independently
            // reconstructible geometry and complete type graph.
            _ => return shape(current, declarations, cache, active).map(|_| ()),
        }
    }
    Err(InvalidStructuralShape)
}

fn conventional_sum_shape(
    common_fields: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, InvalidStructuralShape> {
    conventional_sum_layout(common_fields, cases, declarations, cache, active)
        .map(|layout| layout.shape)
}

fn conventional_sum_layout(
    common_fields: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ConventionalSumLayout, InvalidStructuralShape> {
    let common = common_fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| field_shape(&field.field_type, declarations, cache, active))
        .collect::<Result<Vec<_>, _>>()?;
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| field_shape(&field.field_type, declarations, cache, active))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
        .map_err(|_| InvalidStructuralShape)
}

fn field_shape(
    field: &StructuralFieldType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, InvalidStructuralShape> {
    if matches!(field, StructuralFieldType::Atomic(_)) && field.atomic_scalar_type().is_none() {
        return Err(InvalidStructuralShape);
    }
    match field {
        StructuralFieldType::RefinedScalar(refinement) => field_shape(
            &StructuralFieldType::Scalar(refinement.carrier()),
            declarations,
            cache,
            active,
        ),
        StructuralFieldType::Scalar(ScalarType::Boolean)
        | StructuralFieldType::Atomic(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
        | StructuralFieldType::Atomic(ScalarType::Integer(integer)) => {
            let bytes = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(
                u32::from(bytes),
                bytes.next_power_of_two().min(16),
            ))
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            let bytes = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(
                u32::from(bytes),
                bytes.next_power_of_two().min(16),
            ))
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::Atomic(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::Atomic(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(ValueShape::float(8)),
        StructuralFieldType::Structural(nested) => shape(*nested, declarations, cache, active),
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView { .. })
        | StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BorrowedView { .. },
            ..
        } => Ok(ValueShape::integer(16, 8)),
        // The bound lives in the declaration; the stored descriptor is the
        // bare referent pointer.
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedView { .. })
        | StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BoundedView { .. },
            ..
        } => Ok(ValueShape::integer(8, 8)),
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity })
        | StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BoundedOwned { capacity },
            ..
        } => {
            let bytes = capacity.checked_add(8).ok_or(InvalidStructuralShape)?;
            Ok(ValueShape::integer(
                u32::try_from(bytes).map_err(|_| InvalidStructuralShape)?,
                8,
            ))
        }
        StructuralFieldType::Erased { .. } => Err(InvalidStructuralShape),
    }
}

fn align(value: u32, alignment: u32) -> Result<u32, InvalidStructuralShape> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or(InvalidStructuralShape)
}

pub(super) fn scalar_shape(scalar: ScalarType) -> ValueShape {
    match scalar {
        ScalarType::Boolean => ValueShape::integer(1, 1),
        ScalarType::Integer(integer) => {
            let bytes = integer.bits().div_ceil(8);
            ValueShape::integer(u32::from(bytes), bytes.next_power_of_two().min(8))
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
    }
}
