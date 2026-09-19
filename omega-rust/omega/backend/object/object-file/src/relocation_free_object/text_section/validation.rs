//! Direct source-to-object correspondence, independent of symbol construction.

use crate::{
    RelocationFreeObjectFromTextError, RelocationFreeObjectPlan,
    RelocationFreeObjectRelocationRequirements, RelocationFreeTextSectionPlacement,
    TextSectionRelocationRequirements, validate_relocation_free_object,
};
use optimization_core::OptimizationSelectionIdentity;
pub fn validate_relocation_free_object_from_text(
    text: &RelocationFreeTextSectionPlacement,
    selections: OptimizationSelectionIdentity,
    object: &RelocationFreeObjectPlan,
) -> Result<(), RelocationFreeObjectFromTextError> {
    validate_relocation_free_object(object)
        .map_err(RelocationFreeObjectFromTextError::InvalidObject)?;
    if object.source_text_section != text.identity
        || object.psi != text.psi
        || object.fuel_schedule != text.fuel_schedule
        || object.selected != text.selected
        || object.selections != selections
        || object.target != text.target
        || object.text_section.alignment != text.section_alignment
        || object.text_section.byte_count != text.byte_count
        || object.text_section.bytes != text.bytes
        || object.semantic_entry != text.semantic_entry
        || object.symbols.len() != text.functions.len()
        || object.unresolved_normalized_foreign_calls.len()
            != text.unresolved_normalized_foreign_calls.len()
    {
        return Err(RelocationFreeObjectFromTextError::SourceMismatch);
    }
    if !matches!(
        (object.relocation_requirements, text.relocation_requirements),
        (
            RelocationFreeObjectRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        ) | (
            RelocationFreeObjectRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1,
            TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1,
        )
    ) {
        return Err(RelocationFreeObjectFromTextError::SourceMismatch);
    }
    // Canonical object admission above fixes ordinal, name, role and linkage.
    // This join checks the source facts those independently canonical rows claim.
    for (symbol, function) in object.symbols.iter().zip(&text.functions) {
        if symbol.source_function_index != function.source_function_index
            || symbol.machine != function.machine
            || symbol.section_offset != function.section_offset
            || symbol.byte_count != function.byte_count
            || (symbol.machine == text.semantic_entry
                && symbol.section_offset != text.semantic_entry_offset)
        {
            return Err(RelocationFreeObjectFromTextError::SourceMismatch);
        }
    }
    // The import plan names exactly the distinct roster coordinates the source
    // rows carry, in canonical sorted order; every bound field row replays its
    // placement evidence verbatim against its declared import symbol.
    let mut expected_coordinates = text
        .unresolved_normalized_foreign_calls
        .iter()
        .map(|call| (call.boundary, call.ordinal))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter();
    for import in &object.normalized_imports {
        if expected_coordinates.next() != Some((import.boundary, import.ordinal)) {
            return Err(RelocationFreeObjectFromTextError::SourceMismatch);
        }
    }
    if expected_coordinates.next().is_some() {
        return Err(RelocationFreeObjectFromTextError::SourceMismatch);
    }
    for (field, source) in object
        .unresolved_normalized_foreign_calls
        .iter()
        .zip(&text.unresolved_normalized_foreign_calls)
    {
        if field.resolution != *source {
            return Err(RelocationFreeObjectFromTextError::SourceMismatch);
        }
    }
    Ok(())
}
