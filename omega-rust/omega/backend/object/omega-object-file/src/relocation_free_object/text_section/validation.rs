//! Direct source-to-object correspondence, independent of symbol construction.
use super::*;

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
    {
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
    Ok(())
}
