//! Producer-independent structural shape reconstruction for the admitted ABI closure.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::ValueShape;
use psi_core::{IeeeFloatFormat, ScalarType, StructuralTypeId};
use psi_terminal::{
    ByteSequenceCarrier, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

use super::StructuralCallReturnProjectedQualificationValidationError as Error;

pub(super) fn reconstruct(
    root: StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> Result<ValueShape, Error> {
    let declarations = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    shape(
        root,
        &declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

fn shape(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, Error> {
    if let Some(shape) = cache.get(&structural_type) {
        return Ok(*shape);
    }
    if !active.insert(structural_type) {
        return Err(Error::SourceShape);
    }
    let declaration = declarations
        .get(&structural_type)
        .ok_or(Error::SourceShape)?;
    let result = match &declaration.shape {
        StructuralTypeShape::Record { fields } if !fields.is_empty() => {
            let mut byte_size = 0_u32;
            let mut alignment = 1_u16;
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                let field_shape = field_shape(&field.field_type, declarations, cache, active)?;
                alignment = alignment.max(field_shape.alignment);
                byte_size = align(byte_size, u32::from(field_shape.alignment))?;
                byte_size = byte_size
                    .checked_add(u32::from(field_shape.byte_size))
                    .ok_or(Error::SourceShape)?;
            }
            byte_size = align(byte_size, u32::from(alignment))?;
            ValueShape::integer(
                u16::try_from(byte_size).map_err(|_| Error::SourceShape)?,
                alignment,
            )
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            let element = shape(*element, declarations, cache, active)?;
            let stride = align(u32::from(element.byte_size), u32::from(element.alignment))?;
            let bytes = u64::from(stride)
                .checked_mul(*length)
                .and_then(|value| u16::try_from(value).ok())
                .ok_or(Error::SourceShape)?;
            ValueShape::integer(bytes, element.alignment)
        }
        StructuralTypeShape::Sum { cases } if !cases.is_empty() => {
            conventional_sum_shape(&[], cases, declarations, cache, active)?
        }
        StructuralTypeShape::Mixed { fields, cases } if !cases.is_empty() => {
            conventional_sum_shape(fields, cases, declarations, cache, active)?
        }
        _ => return Err(Error::SourceShape),
    };
    active.remove(&structural_type);
    cache.insert(structural_type, result);
    Ok(result)
}

fn conventional_sum_shape(
    common_fields: &[psi_terminal::StructuralFieldDeclaration],
    cases: &[psi_terminal::StructuralCaseDeclaration],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, Error> {
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
    omega_calling_conventions::evaluate_conventional_sum_layout(&common, &payloads)
        .map(|layout| layout.shape)
        .map_err(|_| Error::SourceShape)
}

fn field_shape(
    field: &StructuralFieldType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<ValueShape, Error> {
    match field {
        StructuralFieldType::Scalar(ScalarType::Boolean) => Ok(ValueShape::integer(1, 1)),
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
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
            let bytes = capacity.checked_add(8).ok_or(Error::SourceShape)?;
            Ok(ValueShape::integer(
                u16::try_from(bytes).map_err(|_| Error::SourceShape)?,
                8,
            ))
        }
        StructuralFieldType::Erased { .. } => Err(Error::SourceShape),
    }
}

fn align(value: u32, alignment: u32) -> Result<u32, Error> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or(Error::SourceShape)
}
