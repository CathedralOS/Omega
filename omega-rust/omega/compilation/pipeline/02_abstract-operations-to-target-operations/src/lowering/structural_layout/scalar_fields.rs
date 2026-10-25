//! Exact scalar-field layout, retaining canonical IEEE format identity.

use super::LoweringError;
use semantic_vocabulary::{ScalarType, StructuralFieldId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

/// One static field address, including a selected payload namespace. Case is
/// layout data, not permission: Psi has separately checked the live tag. The
/// field declaration and its physical placement remain paired throughout.
pub(in crate::lowering) fn static_field_geometry(
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: StructuralFieldId,
    expected: ScalarType,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<(u32, u32), LoweringError> {
    let invalid = || LoweringError::UnknownStructuralType(root);
    let (_, declaration) = terminal_semantics::static_field_projection(
        declarations.values().copied(),
        root,
        path,
        field,
    )
    .ok_or_else(invalid)?;
    if declaration.field_type.scalar_type() != Some(expected)
        || matches!(
            declaration.storage.as_ref(),
            Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(_))
        )
    {
        return Err(invalid());
    }
    let (prefix, selected_case) = match path.split_last() {
        Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
        _ => (path, None),
    };
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    let (carrier, _, offset, indices) =
        super::runtime_projection(root, prefix, declarations, &mut cache, &mut active)?;
    if !indices.is_empty() {
        return Err(invalid());
    }
    let (displacement, bytes) = if let Some(case) = selected_case {
        let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
            &declarations.get(&carrier).ok_or_else(invalid)?.shape
        else {
            return Err(invalid());
        };
        let case_position = cases
            .iter()
            .position(|candidate| candidate.id == case)
            .ok_or_else(invalid)?;
        let field_position = cases[case_position]
            .fields
            .iter()
            .filter(|candidate| !candidate.relevance.is_erased())
            .position(|candidate| candidate.id == field)
            .ok_or_else(invalid)?;
        let layout = super::structural_sum_layout(carrier, declarations, &mut cache, &mut active)?;
        let placed = layout.cases[case_position].fields[field_position];
        (placed.byte_offset, placed.shape.byte_size)
    } else {
        direct_scalar_field_geometry(carrier, field, expected, declarations)?
    };
    Ok((offset.checked_add(displacement).ok_or_else(invalid)?, bytes))
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
                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
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
    let Some(fields) = declaration.shape.record_fields() else {
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
            if matches!(candidate.field_type, (StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)))
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
