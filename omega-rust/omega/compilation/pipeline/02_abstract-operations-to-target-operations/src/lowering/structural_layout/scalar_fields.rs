//! Exact scalar-field layout, retaining canonical IEEE format identity.

use super::LoweringError;
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};
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
    if declarations
        .get(&structural_type)
        .is_some_and(|declaration| {
            let fields = match &declaration.shape {
                StructuralTypeShape::Record { fields, .. }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return false,
            };
            fields.iter().any(|candidate| {
                candidate.id == field
                    && matches!(
                        candidate.storage.as_ref(),
                        Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
                    )
            })
        })
    {
        // A full-container access envelope is not a direct scalar address.
        return Err(LoweringError::UnknownStructuralType(structural_type));
    }
    direct_scalar_field_geometry(structural_type, field, expected_type, declarations)
        .map(|(offset, _)| offset)
}

/// Physical field geometry is independent of the scalar register carrier.
/// Authored scalar storage can occupy fewer bytes than its semantic type.
pub(in crate::lowering) fn direct_scalar_field_geometry(
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
    expected_type: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(u32, u32), LoweringError> {
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
        .map(|common| (common.byte_offset, common.shape.byte_size))
        .ok_or(LoweringError::UnknownStructuralType(structural_type));
    }
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
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
        if candidate.storage.is_some() && candidate.id != field {
            continue;
        }
        if candidate.id == field
            && let Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) =
                candidate.storage.as_ref()
        {
            if matches!(candidate.field_type, StructuralFieldType::Atomic(_))
                || candidate.field_type.scalar_type() != Some(expected_type)
            {
                return Err(LoweringError::UnknownStructuralType(structural_type));
            }
            // This is the full-container access envelope, not a field ABI
            // shape or a contiguous scalar pointer. Projection rejects that
            // latter interpretation because fragmented storage has no offset.
            let (start, width) = fragments
                .byte_envelope()
                .ok_or(LoweringError::UnknownStructuralType(structural_type))?;
            return Ok((
                u32::try_from(start)
                    .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
                u32::try_from(width)
                    .map_err(|_| LoweringError::StructuralTypeTooLarge(structural_type))?,
            ));
        }
        let shape = super::structural_field_storage_shape(
            candidate,
            structural_type,
            declarations,
            &mut cache,
            &mut active,
        )?;
        offset =
            super::structural_field_offset(candidate, offset, shape.alignment, structural_type)?;
        if candidate.id == field {
            // This is layout, not store admission: current-IR validation separately
            // binds restricted writes to their own accepted obligations.
            return (candidate.field_type.scalar_type() == Some(expected_type))
                .then_some((offset, shape.byte_size))
                .ok_or(LoweringError::UnknownStructuralType(structural_type));
        }
        offset = offset
            .checked_add(shape.byte_size)
            .ok_or(LoweringError::StructuralTypeTooLarge(structural_type))?;
    }
    Err(LoweringError::UnknownStructuralType(structural_type))
}
