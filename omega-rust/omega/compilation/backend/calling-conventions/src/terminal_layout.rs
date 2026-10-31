//! Native storage geometry for canonical Terminal declarations.
//! Instruction lowering and task stack planning consume this same owner.
use crate::ValueShape;
use semantic_vocabulary::{IeeeFloatFormat, ScalarApplicationId, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLayoutError {
    RecursiveStructuralType(StructuralTypeId),
    UnknownStructuralType(StructuralTypeId),
    UnclosedTargetLayout(StructuralTypeId),
    UnclosedArrayExtent(StructuralTypeId),
    StructuralTypeTooLarge(StructuralTypeId),
    EmptyStructuralType(StructuralTypeId),
    UnsupportedStructuralSum(StructuralTypeId),
}

pub fn structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, TerminalLayoutError> {
    resolve_structural_shape(
        structural_type,
        declarations,
        cache,
        active,
        &mut reject_unclosed_extent,
    )?
    .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type))
}

/// Discover the scalar inputs required by these physical storage roots using
/// the same geometry walk as lowering. References and views store descriptors;
/// their referents are separate roots only when an access demands their layout.
/// Unknown extents do not invent sizes or prevent discovery in sibling fields.
pub fn required_scalar_extents(
    roots: impl IntoIterator<Item = StructuralTypeId>,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<BTreeSet<ScalarApplicationId>, TerminalLayoutError> {
    let mut required = BTreeSet::new();
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    for root in roots {
        resolve_structural_shape(
            root,
            declarations,
            &mut cache,
            &mut active,
            &mut |_, application| {
                required.insert(application);
                Ok(None)
            },
        )?;
    }
    Ok(required)
}

fn reject_unclosed_extent(
    owner: StructuralTypeId,
    _: ScalarApplicationId,
) -> Result<Option<u64>, TerminalLayoutError> {
    Err(TerminalLayoutError::UnclosedArrayExtent(owner))
}

fn resolve_structural_shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    extent: &mut impl FnMut(
        StructuralTypeId,
        ScalarApplicationId,
    ) -> Result<Option<u64>, TerminalLayoutError>,
) -> Result<Option<ValueShape>, TerminalLayoutError> {
    if let Some(shape) = cache.get(&structural_type) {
        return Ok(Some(*shape));
    }
    if !active.insert(structural_type) {
        return Err(TerminalLayoutError::RecursiveStructuralType(
            structural_type,
        ));
    }
    let result = (|| {
        let declaration = declarations
            .get(&structural_type)
            .copied()
            .ok_or(TerminalLayoutError::UnknownStructuralType(structural_type))?;
        match &declaration.shape {
            // Established Fused bindings have no payload. This describes only
            // geometry; their marker is not an empty-record constructor.
            StructuralTypeShape::BindingCarrier => Ok(Some(ValueShape::integer(0, 1))),
            // All supported native ABIs use 64-bit addresses. The carrier
            // stores the referent's address, never its contents; custody is
            // checked separately. Nested reference leaves use the same layout
            // as whole results, so enclosing record offsets remain coherent.
            StructuralTypeShape::Reference { referent, .. } => Ok(Some(ValueShape::integer(
                if scalar_storage_type(*referent, declarations).is_some() {
                    runtime_abi::ScalarReferenceLayout::NATIVE.byte_size
                } else {
                    8
                },
                8,
            ))),
            StructuralTypeShape::PrimitiveScalar(scalar) => Ok(Some(scalar_shape(*scalar))),
            StructuralTypeShape::ByteSequence(carrier) => {
                byte_sequence_shape(*carrier, structural_type).map(Some)
            }
            // A borrowed element view is a base+extent descriptor, the same
            // two-word carrier as `ByteSequence::BorrowedView`.
            StructuralTypeShape::ElementView { .. } => Ok(Some(ValueShape::integer(16, 8))),
            StructuralTypeShape::Record { fields, storage } => {
                if matches!(
                    storage,
                    terminal_psi::StructuralRecordStorage::TargetLayout(_)
                ) {
                    return Err(TerminalLayoutError::UnclosedTargetLayout(structural_type));
                }
                if let terminal_psi::StructuralRecordStorage::Authored(storage)
                | terminal_psi::StructuralRecordStorage::AccessView(storage)
                | terminal_psi::StructuralRecordStorage::Accessor(storage) = storage
                {
                    return Ok(Some(ValueShape::integer(
                        u32::try_from(storage.byte_size).map_err(|_| {
                            TerminalLayoutError::StructuralTypeTooLarge(structural_type)
                        })?,
                        u16::try_from(storage.byte_alignment).map_err(|_| {
                            TerminalLayoutError::StructuralTypeTooLarge(structural_type)
                        })?,
                    )));
                }
                let mut byte_size = 0_u32;
                let mut alignment = 1_u16;
                let mut complete = true;
                for field in fields {
                    // Authored relevance and physical carrier erasure are
                    // independent: a Fused service remains semantically relevant.
                    if field.relevance.is_erased()
                        || matches!(field.field_type, StructuralFieldType::Erased { .. })
                    {
                        continue;
                    }
                    let field_shape = resolve_structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        cache,
                        active,
                        extent,
                    )?;
                    let Some(field_shape) = field_shape else {
                        complete = false;
                        continue;
                    };
                    if !complete {
                        continue;
                    }
                    alignment = alignment.max(field_shape.alignment);
                    byte_size =
                        checked_align_up_u32(byte_size, u32::from(field_shape.alignment))
                            .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                    byte_size = byte_size
                        .checked_add(field_shape.byte_size)
                        .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                }
                if !complete {
                    return Ok(None);
                }
                byte_size = checked_align_up_u32(byte_size, u32::from(alignment))
                    .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                Ok(Some(ValueShape::integer(byte_size, alignment)))
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let length = match length {
                    terminal_psi::ArrayExtent::Literal(length) => Some(*length),
                    terminal_psi::ArrayExtent::Application(application) => {
                        extent(structural_type, *application)?
                    }
                };
                // The complete primitive-array chain is still checked below
                // empty dimensions. Its canonical zero-byte ABI must not need
                // an addressable element layout or overflow an unused extent.
                if empty_primitive_array(structural_type, declarations) {
                    return Ok(Some(ValueShape::integer(0, 1)));
                }
                let element =
                    resolve_structural_shape(*element, declarations, cache, active, extent)?;
                let (Some(length), Some(element)) = (length, element) else {
                    return Ok(None);
                };
                let stride = checked_align_up_u32(element.byte_size, u32::from(element.alignment))
                    .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                let byte_size = u64::from(stride)
                    .checked_mul(length)
                    .and_then(|size| u32::try_from(size).ok())
                    .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?;
                Ok(Some(ValueShape::integer(
                    byte_size,
                    if length == 0 { 1 } else { element.alignment },
                )))
            }
            StructuralTypeShape::Sum { cases } => conventional_sum_layout_from_parts(
                structural_type,
                &[],
                cases,
                declarations,
                cache,
                active,
                extent,
            )
            .map(|layout| layout.map(|layout| layout.shape)),
            StructuralTypeShape::Mixed { fields, cases } => conventional_sum_layout_from_parts(
                structural_type,
                fields,
                cases,
                declarations,
                cache,
                active,
                extent,
            )
            .map(|layout| layout.map(|layout| layout.shape)),
        }
    })();
    active.remove(&structural_type);
    let shape = result?;
    if let Some(shape) = shape {
        cache.insert(structural_type, shape);
    }
    Ok(shape)
}

pub fn scalar_storage_type(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<ScalarType> {
    let StructuralTypeShape::PrimitiveScalar(scalar) = declarations.get(&structural_type)?.shape
    else {
        return None;
    };
    runtime_abi::ScalarStorageCodec::from_scalar(scalar).map(|_| scalar)
}

pub fn structural_sum_layout(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<crate::ConventionalSumLayout, TerminalLayoutError> {
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(TerminalLayoutError::UnknownStructuralType(structural_type))?;
    match &declaration.shape {
        StructuralTypeShape::Sum { cases } => conventional_sum_layout_from_parts(
            structural_type,
            &[],
            cases,
            declarations,
            cache,
            active,
            &mut reject_unclosed_extent,
        )?
        .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type)),
        StructuralTypeShape::Mixed { fields, cases } => conventional_sum_layout_from_parts(
            structural_type,
            fields,
            cases,
            declarations,
            cache,
            active,
            &mut reject_unclosed_extent,
        )?
        .ok_or(TerminalLayoutError::UnclosedArrayExtent(structural_type)),
        _ => Err(TerminalLayoutError::UnsupportedStructuralSum(
            structural_type,
        )),
    }
}

fn conventional_sum_layout_from_parts(
    structural_type: StructuralTypeId,
    common_fields: &[terminal_psi::StructuralFieldDeclaration],
    cases: &[terminal_psi::StructuralCaseDeclaration],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    extent: &mut impl FnMut(
        StructuralTypeId,
        ScalarApplicationId,
    ) -> Result<Option<u64>, TerminalLayoutError>,
) -> Result<Option<crate::ConventionalSumLayout>, TerminalLayoutError> {
    if cases.is_empty() {
        return Err(TerminalLayoutError::EmptyStructuralType(structural_type));
    }
    let common = common_fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .map(|field| {
            resolve_structural_field_shape(
                &field.field_type,
                structural_type,
                declarations,
                cache,
                active,
                extent,
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
                    resolve_structural_field_shape(
                        &field.field_type,
                        structural_type,
                        declarations,
                        cache,
                        active,
                        extent,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let Some(common) = common.into_iter().collect::<Option<Vec<_>>>() else {
        return Ok(None);
    };
    let Some(payloads) = payloads
        .into_iter()
        .map(|fields| fields.into_iter().collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    crate::evaluate_conventional_sum_layout(&common, &payloads)
        .map(Some)
        .map_err(|_| TerminalLayoutError::StructuralTypeTooLarge(structural_type))
}

pub fn structural_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, TerminalLayoutError> {
    resolve_structural_field_shape(
        field,
        owner,
        declarations,
        cache,
        active,
        &mut reject_unclosed_extent,
    )?
    .ok_or(TerminalLayoutError::UnclosedArrayExtent(owner))
}

fn resolve_structural_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    extent: &mut impl FnMut(
        StructuralTypeId,
        ScalarApplicationId,
    ) -> Result<Option<u64>, TerminalLayoutError>,
) -> Result<Option<ValueShape>, TerminalLayoutError> {
    if let StructuralFieldType::Structural(nested) = field {
        return resolve_structural_shape(*nested, declarations, cache, active, extent);
    }
    scalar_field_shape(field, owner).map(Some)
}

fn scalar_field_shape(
    field: &StructuralFieldType,
    owner: StructuralTypeId,
) -> Result<ValueShape, TerminalLayoutError> {
    if matches!(
        field,
        StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)
    ) && field.atomic_scalar_type().is_none()
    {
        return Err(TerminalLayoutError::UnknownStructuralType(owner));
    }
    match field {
        StructuralFieldType::RefinedScalar(refinement) => {
            scalar_field_shape(&StructuralFieldType::Scalar(refinement.carrier()), owner)
        }
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
        StructuralFieldType::BoundedInteger(bounds)
        | StructuralFieldType::AtomicBoundedInteger(bounds) => {
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
        StructuralFieldType::Structural(_) | StructuralFieldType::Erased { .. } => {
            // Callers filter erased fields before requesting runtime shape.
            Err(TerminalLayoutError::UnknownStructuralType(owner))
        }
    }
}

pub fn byte_sequence_shape(
    carrier: terminal_psi::ByteSequenceCarrier,
    structural_type: StructuralTypeId,
) -> Result<ValueShape, TerminalLayoutError> {
    let byte_size = match carrier {
        // Current native targets are 64-bit. The semantic carrier
        // deliberately does not retain the physical descriptor fields.
        terminal_psi::ByteSequenceCarrier::BorrowedView { .. } => 16_u64,
        // A bounded view's extent lives in the declaration, so the descriptor
        // is the bare referent pointer.
        terminal_psi::ByteSequenceCarrier::BoundedView { .. } => 8_u64,
        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => capacity
            .checked_add(8)
            .ok_or(TerminalLayoutError::StructuralTypeTooLarge(structural_type))?,
    };
    Ok(ValueShape::integer(
        u32::try_from(byte_size)
            .map_err(|_| TerminalLayoutError::StructuralTypeTooLarge(structural_type))?,
        8,
    ))
}

pub fn checked_align_up_u32(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

/// Storage geometry of one primitive scalar in the supported native ABI.
pub fn scalar_shape(scalar: ScalarType) -> ValueShape {
    match scalar {
        ScalarType::Boolean => ValueShape::integer(1, 1),
        ScalarType::Integer(integer) => {
            let size = integer.bits().div_ceil(8);
            ValueShape::integer(u32::from(size), size.next_power_of_two().min(8))
        }
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
    }
}

fn empty_primitive_array(
    mut current: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> bool {
    let mut empty = false;
    for _ in 0..declarations.len() {
        match declarations
            .get(&current)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::FixedArray { element, length }) => {
                let Some(length) = length.literal() else {
                    return false;
                };
                empty |= length == 0;
                current = *element;
            }
            Some(StructuralTypeShape::PrimitiveScalar(scalar)) => {
                return empty && runtime_abi::ScalarStorageCodec::from_scalar(*scalar).is_some();
            }
            _ => return false,
        }
    }
    false
}
