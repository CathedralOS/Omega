//! Exact scalar-field layout, retaining canonical IEEE format identity.

use super::{LoweringError, checked_align_up_u32};
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
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    // A mixed shape's common field sits after the tag at the offset the
    // conventional sum layout assigns it, the same in every case.
    if let StructuralTypeShape::Mixed { fields, .. } = &declaration.shape {
        let (position, candidate) = fields
            .iter()
            .filter(|candidate| !candidate.relevance.is_erased())
            .enumerate()
            .find(|(_, candidate)| candidate.id == field)
            .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
        if candidate.field_type.scalar_type() != Some(expected_type) {
            return Err(LoweringError::UnknownStructuralType(structural_type));
        }
        return super::structural_sum_layout(
            structural_type,
            declarations,
            &mut cache,
            &mut active,
        )?
        .common_fields
        .get(position)
        .map(|common| common.byte_offset)
        .ok_or(LoweringError::UnknownStructuralType(structural_type));
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(LoweringError::UnknownStructuralType(structural_type));
    };
    let mut offset = 0_u32;
    for candidate in fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
    {
        if matches!(candidate.field_type, StructuralFieldType::Erased { .. }) {
            continue;
        }
        let shape = super::structural_field_shape(
            &candidate.field_type,
            structural_type,
            declarations,
            &mut cache,
            &mut active,
        )?;
        offset = checked_align_up_u32(offset, u32::from(shape.alignment))
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
        if candidate.id == field {
            // This is layout, not store admission: current-IR validation separately
            // binds restricted writes to their own accepted obligations.
            return (candidate.field_type.scalar_type() == Some(expected_type))
                .then_some(offset)
                .ok_or(LoweringError::UnknownStructuralType(structural_type));
        }
        offset = offset
            .checked_add(shape.byte_size)
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
    }
    Err(LoweringError::UnknownStructuralType(structural_type))
}
