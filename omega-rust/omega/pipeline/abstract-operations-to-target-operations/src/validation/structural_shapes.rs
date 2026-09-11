//! Producer-independent referent shape reconstruction for structural ABI validation.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::ValueShape;
use semantic_vocabulary::{IeeeFloatFormat, ScalarType, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
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
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar_shape(*scalar),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView) => {
            ValueShape::integer(16, 8)
        }
        StructuralTypeShape::Record { fields } => {
            let mut byte_size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
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
        .map(|layout| layout.shape)
        .map_err(|_| InvalidStructuralShape)
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
