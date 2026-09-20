//! Producer-independent referent shape reconstruction for structural ABI validation.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::{IeeeFloatFormat, OperationId, ScalarType, StructuralTypeId};
use target_operations::{
    TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct InvalidStructuralShape;

#[cfg(test)]
mod tests;

pub(super) fn reconstruct(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<ValueShape, InvalidStructuralShape> {
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    shape(root, &indexed, &mut BTreeMap::new(), &mut BTreeSet::new())
}

pub(super) fn parameter_shape(referent: ValueShape, access: StructuralAccess) -> ValueShape {
    match access {
        StructuralAccess::Owned => referent,
        StructuralAccess::SharedBorrow
        | StructuralAccess::MutableBorrow
        | StructuralAccess::WriteOnlyBorrow => {
            ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
        }
    }
}

pub(super) fn project_static_path(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Result<(StructuralTypeId, u32), InvalidStructuralShape> {
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut byte_offset = 0_u32;
    for segment in path {
        if let StructuralPathSegment::FixedIndex(index) = segment {
            let declaration = indexed
                .get(&structural_type)
                .ok_or(InvalidStructuralShape)?;
            let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                return Err(InvalidStructuralShape);
            };
            if *index >= length {
                return Err(InvalidStructuralShape);
            }
            let element_shape = shape(element, &indexed, &mut cache, &mut active)?;
            let stride = align(
                u32::from(element_shape.byte_size),
                u32::from(element_shape.alignment),
            )?;
            let element_offset = u64::from(stride)
                .checked_mul(*index)
                .and_then(|offset| u32::try_from(offset).ok())
                .ok_or(InvalidStructuralShape)?;
            byte_offset = byte_offset
                .checked_add(element_offset)
                .ok_or(InvalidStructuralShape)?;
            structural_type = element;
            continue;
        }
        let StructuralPathSegment::Field(identity) = segment else {
            return Err(InvalidStructuralShape);
        };
        let declaration = indexed
            .get(&structural_type)
            .ok_or(InvalidStructuralShape)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(InvalidStructuralShape);
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for field in fields.iter().filter(|field| {
            !field.relevance.is_erased()
                && !matches!(field.field_type, StructuralFieldType::Erased { .. })
        }) {
            let shape = field_shape(&field.field_type, &indexed, &mut cache, &mut active)?;
            local_offset = align(local_offset, u32::from(shape.alignment))?;
            if field.identity == *identity {
                let field_type = match &field.field_type {
                    StructuralFieldType::Structural(nested) => *nested,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape().ok_or(InvalidStructuralShape)?;
                        *indexed
                            .iter()
                            .find(|(_, declaration)| declaration.shape == shape)
                            .map(|(id, _)| id)
                            .ok_or(InvalidStructuralShape)?
                    }
                };
                selected = Some((field_type, local_offset));
                break;
            }
            local_offset = local_offset
                .checked_add(u32::from(shape.byte_size))
                .ok_or(InvalidStructuralShape)?;
        }
        let (field_type, field_offset) = selected.ok_or(InvalidStructuralShape)?;
        byte_offset = byte_offset
            .checked_add(field_offset)
            .ok_or(InvalidStructuralShape)?;
        structural_type = field_type;
    }
    Ok((structural_type, byte_offset))
}

/// The call-argument projection: every segment names a record field and the
/// projected shape is the selected field's own storage shape. Leaf fields keep
/// their natural declaration shape resolved through the canonical leaf
/// catalog; the returned shape is not the catalog's normalized scalar shape,
/// matching the producer's `resolve_structural_field_path` replay exactly.
pub(super) fn projected_field(
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Result<(StructuralTypeId, ValueShape, u32), InvalidStructuralShape> {
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut byte_offset = 0_u32;
    let mut selected_shape = None;
    for segment in path {
        let StructuralPathSegment::Field(identity) = segment else {
            return Err(InvalidStructuralShape);
        };
        let declaration = indexed
            .get(&structural_type)
            .ok_or(InvalidStructuralShape)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return Err(InvalidStructuralShape);
        };
        let mut local_offset = 0_u32;
        let mut selected = None;
        for field in fields.iter().filter(|field| {
            !field.relevance.is_erased()
                && !matches!(field.field_type, StructuralFieldType::Erased { .. })
        }) {
            let shape = field_shape(&field.field_type, &indexed, &mut cache, &mut active)?;
            local_offset = align(local_offset, u32::from(shape.alignment))?;
            if field.identity == *identity {
                let field_type = match &field.field_type {
                    StructuralFieldType::Structural(nested) => *nested,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape().ok_or(InvalidStructuralShape)?;
                        *indexed
                            .iter()
                            .find(|(_, declaration)| declaration.shape == shape)
                            .map(|(id, _)| id)
                            .ok_or(InvalidStructuralShape)?
                    }
                };
                selected = Some((field_type, shape, local_offset));
                break;
            }
            local_offset = local_offset
                .checked_add(u32::from(shape.byte_size))
                .ok_or(InvalidStructuralShape)?;
        }
        let (field_type, shape, field_offset) = selected.ok_or(InvalidStructuralShape)?;
        byte_offset = byte_offset
            .checked_add(field_offset)
            .ok_or(InvalidStructuralShape)?;
        structural_type = field_type;
        selected_shape = Some(shape);
    }
    selected_shape
        .map(|shape| (structural_type, shape, byte_offset))
        .ok_or(InvalidStructuralShape)
}

/// The array transport metadata one indexed projection retains: the referent
/// root's own `FixedArray` extent and aligned element stride, or nothing when
/// the root is not an array. The producer derives the same pair from this
/// declaration walk; the retained fields are evidence, never indexing
/// authority.
pub(super) fn root_array_transport(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<(Option<u64>, Option<u32>), InvalidStructuralShape> {
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    let Some(declaration) = indexed.get(&root).copied() else {
        return Err(InvalidStructuralShape);
    };
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return Ok((None, None));
    };
    let element_shape = shape(
        element,
        &indexed,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let stride = align(
        u32::from(element_shape.byte_size),
        u32::from(element_shape.alignment),
    )?;
    Ok((Some(length), Some(stride)))
}

/// A bounded inline byte field has no projected carrier identity: the path's
/// last segment names a record field carrying `ByteSequence(BoundedOwned)`
/// storage. Reconstruct the field's offset and declared capacity so an argument
/// presenting it as a borrowed view replays geometry, not a claimed type.
pub(super) fn bounded_byte_field_geometry(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Result<(u32, u64), InvalidStructuralShape> {
    let Some((StructuralPathSegment::Field(identity), prefix)) = path.split_last() else {
        return Err(InvalidStructuralShape);
    };
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let (parent, parent_offset) = if prefix.is_empty() {
        (root, 0)
    } else {
        project_static_path(root, prefix, declarations)?
    };
    let declaration = indexed.get(&parent).ok_or(InvalidStructuralShape)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(InvalidStructuralShape);
    };
    let mut local_offset = 0_u32;
    for field in fields.iter().filter(|field| {
        !field.relevance.is_erased()
            && !matches!(field.field_type, StructuralFieldType::Erased { .. })
    }) {
        let field_shape = field_shape(&field.field_type, &indexed, &mut cache, &mut active)?;
        local_offset = align(local_offset, u32::from(field_shape.alignment))?;
        if field.identity == *identity {
            let StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
                field.field_type
            else {
                return Err(InvalidStructuralShape);
            };
            let field_offset = parent_offset
                .checked_add(local_offset)
                .ok_or(InvalidStructuralShape)?;
            // The live length word plus declared capacity must remain inside
            // the root storage the argument points into.
            if u64::from(field_offset)
                .checked_add(8)
                .and_then(|end| end.checked_add(capacity))
                .is_none_or(|end| {
                    end > u64::from(
                        shape(root, &indexed, &mut cache, &mut active)
                            .map(|shape| shape.byte_size)
                            .unwrap_or(0),
                    )
                })
            {
                return Err(InvalidStructuralShape);
            }
            return Ok((field_offset, capacity));
        }
        local_offset = local_offset
            .checked_add(u32::from(field_shape.byte_size))
            .ok_or(InvalidStructuralShape)?;
    }
    Err(InvalidStructuralShape)
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
        // A reference carrier transports no referent storage: custody is
        // compile-time metadata, so its value shape is the empty aggregate
        // slot the lowering assigns rather than a pointer-sized payload.
        StructuralTypeShape::Reference { .. } => ValueShape::integer(0, 1),
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_shape(*scalar),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView) => {
            ValueShape::integer(16, 8)
        }
        StructuralTypeShape::Record { fields } => {
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
                    .checked_add(u32::from(field_shape.byte_size))
                    .ok_or(InvalidStructuralShape)?;
            }
            byte_size = align(byte_size, u32::from(alignment))?;
            ValueShape::integer(
                u16::try_from(byte_size).map_err(|_| InvalidStructuralShape)?,
                alignment,
            )
        }
        StructuralTypeShape::FixedArray { length: 0, .. } => {
            // Zero-byte calling plans retain a canonical shape, but emptiness
            // cannot hide an unknown, recursive or nonprimitive element chain.
            validate_empty_primitive_array(structural_type, declarations)?;
            ValueShape::integer(0, 1)
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let element = shape(*element, declarations, cache, active)?;
            let stride = align(u32::from(element.byte_size), u32::from(element.alignment))?;
            let bytes = u64::from(stride)
                .checked_mul(*length)
                .and_then(|value| u16::try_from(value).ok())
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

fn validate_empty_primitive_array(
    root: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
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
            _ => return Err(InvalidStructuralShape),
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

/// The exact durable home one structural-result `Call` must require, replayed the
/// way the producer derives it: the retained operation-result origin carries
/// the semantic result row verbatim, and the layout is the aggregate shape or
/// the conventional sum layout the declared structural type resolves to.
/// Claimed, linear, qualified, or non-aggregate non-sum results have no honest
/// home, so a retained home on one is forged regardless of how plausible the
/// layout looks.
pub(super) fn structural_result_home(
    operation: OperationId,
    result: &StructuralOperationResult,
    declarations: &[StructuralTypeDeclaration],
) -> Result<TargetStructuralHomeRequirement, InvalidStructuralShape> {
    if !result.claims.is_empty() {
        return Err(InvalidStructuralShape);
    }
    Ok(TargetStructuralHomeRequirement {
        origin: TargetStructuralHomeOrigin::OperationResult {
            operation,
            result: result.clone(),
        },
        layout: result_home_layout(result, declarations)?,
    })
}

/// The retained result home one builtin `BoundarySettlement` must require,
/// replayed the way the producer derives it: the origin carries the semantic
/// result row verbatim and the layout is the conventional sum layout the
/// declared result carrier resolves to. The producer accepts `Sum` and
/// `Mixed` carriers here before the realization lane narrows which sums are
/// legal; a non-sum result carrier has no honest boundary home.
pub(super) fn boundary_result_home(
    operation: OperationId,
    result: &StructuralOperationResult,
    declarations: &[StructuralTypeDeclaration],
) -> Result<TargetStructuralHomeRequirement, InvalidStructuralShape> {
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    let declaration = indexed
        .get(&result.structural_type)
        .copied()
        .ok_or(InvalidStructuralShape)?;
    let (common, cases) = match &declaration.shape {
        StructuralTypeShape::Sum { cases } => (&[][..], cases.as_slice()),
        StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases.as_slice()),
        _ => return Err(InvalidStructuralShape),
    };
    if cases.is_empty() {
        return Err(InvalidStructuralShape);
    }
    let layout = conventional_sum_layout(
        common,
        cases,
        &indexed,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    Ok(TargetStructuralHomeRequirement {
        origin: TargetStructuralHomeOrigin::OperationResult {
            operation,
            result: result.clone(),
        },
        layout: TargetStructuralHomeLayout::Sum(layout),
    })
}

fn result_home_layout(
    result: &StructuralOperationResult,
    declarations: &[StructuralTypeDeclaration],
) -> Result<TargetStructuralHomeLayout, InvalidStructuralShape> {
    if result.multiplicity == StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(InvalidStructuralShape);
    }
    let indexed = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    if indexed.len() != declarations.len() {
        return Err(InvalidStructuralShape);
    }
    match indexed
        .get(&result.structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
            reconstruct(result.structural_type, declarations)
                .map(TargetStructuralHomeLayout::Aggregate)
        }
        Some(StructuralTypeShape::Sum { cases }) => {
            if cases.iter().flat_map(|case| &case.fields).any(|field| {
                !matches!(
                    field.field_type.scalar_type(),
                    Some(ScalarType::Integer(integer))
                        if super::structural_signatures::fixed_native_integer_shape(integer)
                            .is_some()
                )
            }) {
                return Err(InvalidStructuralShape);
            }
            conventional_sum_layout(
                &[],
                cases,
                &indexed,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )
            .map(TargetStructuralHomeLayout::Sum)
        }
        _ => Err(InvalidStructuralShape),
    }
}

fn field_shape(
    field: &StructuralFieldType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, InvalidStructuralShape> {
    match field {
        StructuralFieldType::Scalar(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            let bytes = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(
                bytes,
                bytes.next_power_of_two().min(16),
            ))
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            let integer = bounds.integer_type();
            let bytes = integer.bits().div_ceil(8);
            Ok(ValueShape::integer(
                bytes,
                bytes.next_power_of_two().min(16),
            ))
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(ValueShape::float(4)),
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64))
        | StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(ValueShape::float(8)),
        StructuralFieldType::Structural(nested) => shape(*nested, declarations, cache, active),
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView) => {
            Ok(ValueShape::integer(16, 8))
        }
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => {
            let bytes = capacity.checked_add(8).ok_or(InvalidStructuralShape)?;
            Ok(ValueShape::integer(
                u16::try_from(bytes).map_err(|_| InvalidStructuralShape)?,
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
            ValueShape::integer(bytes, bytes.next_power_of_two().min(8))
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
    }
}
