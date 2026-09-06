//! Independent projection admission over already replayed physical object data.

mod stack;

use super::{Error, attribution, host, source};
use crate::{ObjectArtifact, ObjectFunction};
use object_file::{
    SectionKind, StagedOptimizedRelocationFreeObjectContainer, SymbolKind, SymbolSection,
    entry_symbol_name,
};

pub fn validate_function_fragment_object_artifact(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    artifact: &ObjectArtifact,
) -> Result<(), Error> {
    source::admit(source)?;
    let text = source.source().text_section();
    let layout = &artifact.object.layout;
    if artifact.psi != text.psi
        || artifact.target != text.target
        || artifact.entry != text.semantic_entry
        || artifact.object.target != text.target
        || artifact.text_bytes != text.bytes
        || artifact.relocations.target != text.target
        || !artifact.relocations.record_set.records.is_empty()
        || artifact.x86_feature_profile.is_some()
        || artifact.x86_scalar_fma_provider.is_some()
        || !artifact.data_bytes.is_empty()
        || !artifact.dynamic_conformance_tables.is_empty()
        || !artifact.forwarded_dynamic_descriptor_adapters.is_empty()
        || !artifact.forwarded_dynamic_descriptor_tables.is_empty()
        || !artifact.private_functions.is_empty()
        || !artifact.port_effects.is_empty()
        || !artifact.boundary_settlements.is_empty()
        || !artifact.foreign_calls.is_empty()
        || !layout.normalized_imports.is_empty()
        || !layout.function_symbols.is_empty()
        || layout.sections.len() != 1
        || layout.symbols.len() != text.functions.len()
        || artifact.functions.len() != text.functions.len()
    {
        return Err(Error::Mismatch(
            "shared object root or unsupported effect roster changed",
        ));
    }
    let (_, section) = layout
        .sections
        .iter()
        .next()
        .ok_or(Error::Mismatch("object has no text section"))?;
    if section.kind != SectionKind::Text
        || section.size != text.bytes.len()
        || section.alignment != host(text.section_alignment)?
    {
        return Err(Error::Mismatch("shared object text geometry changed"));
    }
    let fragments = source::fragments(source);
    let mut attribution_cursor = 0usize;
    for ((placed, function), (symbol_handle, symbol)) in text
        .functions
        .iter()
        .zip(&artifact.functions)
        .zip(layout.symbols.iter())
    {
        let (abstracted, targeted) = source::function(source, placed.machine)?;
        let fragment = fragments
            .functions
            .iter()
            .find(|fragment| fragment.machine == placed.machine)
            .ok_or(Error::Mismatch("placed function has no fragment"))?;
        let offset = host(placed.section_offset)?;
        let length = host(placed.byte_count)?;
        let entry = placed.machine == text.semantic_entry;
        let expected_name = if entry {
            entry_symbol_name(text.target)
        } else {
            format!("omega_terminal_machine_{}", placed.machine.get())
        };
        if function.machine != placed.machine
            || function.attachment != fragment.attachment
            || function.provenance != fragment.provenance
            || function.fixed_integer_scalar_abi != targeted.fixed_integer_scalar_abi
            || function.text_offset != offset
            || function.byte_count != length
            || function.symbol != symbol_handle
            || symbol.name != expected_name
            || symbol.offset != offset
            || symbol.size != length
            || symbol.kind != SymbolKind::Function
            || symbol.section != SymbolSection::Section(SectionKind::Text)
            || !symbol.import_library.is_empty()
            || (entry && layout.entry_symbol != symbol_handle)
            || !empty_legacy_records(function)
        {
            return Err(Error::Mismatch(
                "shared object function or symbol differs from current data",
            ));
        }
        let unit = matches!(
            abstracted.result,
            abstract_operations::AbstractFunctionResult::Unit
        );
        let frame = source::frame(source, placed.machine)?;
        let calls = text
            .resolved_internal_machine_calls
            .iter()
            .filter(|call| call.caller == placed.machine)
            .collect::<Vec<_>>();
        stack::validate(
            unit,
            (
                frame.map_or(0, |frame| frame.frame_size_bytes),
                frame.map_or(16, |frame| frame.abi_stack_alignment_bytes),
                frame.is_some_and(|frame| frame.contains_call),
            ),
            text.target.architecture,
            &calls,
            function.unit_stack,
            function.scalar_stack,
            &function.unit_call_stacks,
        )?;
        let start = attribution_cursor;
        while let Some(row) = artifact.semantic_code_attribution.get(attribution_cursor) {
            if row.machine != placed.machine {
                break;
            }
            if offset.checked_add(row.attribution.code_offset) != Some(row.text_offset)
                || row
                    .attribution
                    .code_offset
                    .checked_add(row.attribution.byte_count)
                    .is_none_or(|end| end > length)
            {
                return Err(Error::Mismatch(
                    "shared attribution is outside its function",
                ));
            }
            attribution_cursor += 1;
        }
        let rows = artifact.semantic_code_attribution[start..attribution_cursor]
            .iter()
            .map(|row| row.attribution)
            .collect::<Vec<_>>();
        attribution::validate(fragment, abstracted, &rows)?;
    }
    if attribution_cursor != artifact.semantic_code_attribution.len() {
        return Err(Error::Mismatch(
            "shared object retains a foreign semantic attribution",
        ));
    }
    Ok(())
}

fn empty_legacy_records(function: &ObjectFunction) -> bool {
    function.mixed_structural_scalar_abi.is_none()
        && function.structural_call_scalar_return.is_none()
        && function.unit_scalar_abi.is_none()
        && function.x86_scalar_fma.is_empty()
        && function.x86_scalar_fma_occurrences.is_empty()
        && function.x86_floating_control.is_none()
        && function.scalar_call_stacks.is_empty()
        && function.internal_unit_calls.is_empty()
        && function.internal_unit_scalar_calls.is_empty()
        && function.installed_provider_unit_scalar_calls.is_empty()
        && function.dynamic_calls.is_empty()
        && function.stored_dynamic_calls.is_empty()
        && function.dynamic_parameter_calls.is_empty()
        && function.forwarded_dynamic_parameter_calls.is_empty()
        && function.forwarded_dynamic_descriptor_calls.is_empty()
        && function.unit_scalar_homes.is_empty()
        && function.unit_integer_constants.is_empty()
        && function.unit_affine_scalar_records.is_empty()
        && function.unit_structural_scalar_field_stores.is_empty()
        && function.unit_write_only_primitive_stores.is_empty()
        && function.scalar_structural_scalar_field_stores.is_empty()
        && function.unit_parameters.is_empty()
        && function.unit_parameter_homes.is_empty()
        && function.unit_affine_cleanup.is_none()
        && function.scalar_affine_cleanup.is_none()
        && function.scalar_control_affine_cleanups.is_empty()
        && function.scalar_structural_parameters.is_empty()
        && function.scalar_structural_parameter_homes.is_empty()
        && function.ranked_u32_countdown.is_none()
        && function.structural_return.is_none()
}
