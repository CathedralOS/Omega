//! Exact scalar-field layout, retaining canonical IEEE format identity.

use super::{
    IeeeFloatFormat, LoweringError, byte_sequence_shape, checked_align_up_u32, structural_shape,
};
use calling_conventions::ValueShape;
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};
pub(in crate::lowering) fn direct_boolean_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    direct_scalar_field_offset(structural_type, field, ScalarType::Boolean, declarations)
}

pub(in crate::lowering) fn direct_integer_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    scalar_type: semantic_vocabulary::IntegerType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    direct_scalar_field_offset(
        structural_type,
        field,
        ScalarType::Integer(scalar_type),
        declarations,
    )
}

pub(in crate::lowering) fn direct_scalar_field_offset(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    expected_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<u32, LoweringError> {
    let declaration = declarations
        .get(&structural_type)
        .copied()
        .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut offset = 0_u32;
    for candidate in fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
    {
        let shape = match candidate.field_type {
            StructuralFieldType::Scalar(ScalarType::Boolean) => ValueShape::integer(1, 1),
            StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
                let size = integer.bits().div_ceil(8);
                ValueShape::integer(size, size.next_power_of_two().min(16))
            }
            StructuralFieldType::BoundedInteger(bounds) => {
                let integer = bounds.integer_type();
                let size = integer.bits().div_ceil(8);
                ValueShape::integer(size, size.next_power_of_two().min(16))
            }
            StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)) => {
                ValueShape::float(4)
            }
            StructuralFieldType::Scalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)) => {
                ValueShape::float(8)
            }
            StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary32) => ValueShape::float(4),
            StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64) => ValueShape::float(8),
            StructuralFieldType::ByteSequence(carrier) => {
                byte_sequence_shape(carrier, structural_type)?
            }
            StructuralFieldType::Structural(nested) => {
                structural_shape(nested, declarations, &mut cache, &mut active)?
            }
            StructuralFieldType::Erased { .. } => continue,
        };
        offset = checked_align_up_u32(offset, u32::from(shape.alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        if candidate.id == field {
            // This is layout, not store admission: current-IR validation separately
            // binds bounded writes to their own accepted range obligations.
            return (candidate.field_type.scalar_type() == Some(expected_type))
                .then_some(offset)
                .ok_or(LoweringError::UnknownStructuralType(structural_type));
        }
        offset = offset
            .checked_add(u32::from(shape.byte_size))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
    }
    Err(LoweringError::UnknownStructuralType(structural_type))
}
