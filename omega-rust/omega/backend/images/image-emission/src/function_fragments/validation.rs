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
    validate(source, artifact, None)
}

/// Replay the same admission with an expected compiler-private roster: each
/// materialized record must bind one private symbol row, one function-symbol
/// join row, and one carrier whose bytes follow the program text in order.
pub fn validate_function_fragment_object_artifact_with_private_functions(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    artifact: &ObjectArtifact,
    private_functions: &[machine_code::CompilerPrivateMachineCodeFunction],
) -> Result<(), Error> {
    validate(source, artifact, Some(private_functions))
}

fn validate(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    artifact: &ObjectArtifact,
    private_functions: Option<&[machine_code::CompilerPrivateMachineCodeFunction]>,
) -> Result<(), Error> {
    source::admit(source)?;
    let text = source.source().text_section();
    let layout = &artifact.object.layout;
    // The text section covers the program text plus every retained private
    // carrier's own byte span; the roster replay below independently checks
    // each span against the materialized record.
    let expected_text_len =
        artifact
            .private_functions
            .iter()
            .try_fold(text.bytes.len(), |total, carrier| {
                total
                    .checked_add(carrier.function.byte_count)
                    .ok_or(Error::Overflow)
            })?;
    let requires_graph_storage_replay =
        text.functions
            .iter()
            .try_fold(false, |required, placed| -> Result<bool, Error> {
                let (abstracted, _) = source::function(source, placed.machine)?;
                Ok(required || source::requires_graph_storage_replay(&abstracted.operations))
            })?;
    if artifact.requires_graph_storage_replay != requires_graph_storage_replay
        || artifact.psi != text.psi
        || artifact.target != text.target
        || artifact.entry != text.semantic_entry
        || artifact.object.target != text.target
        || artifact.text_bytes.len() != expected_text_len
        || !artifact.text_bytes.starts_with(&text.bytes)
        || artifact.relocations.target != text.target
        || artifact.x86_feature_profile.is_some()
        || artifact.x86_scalar_fma_provider.is_some()
        || !artifact.data_bytes.is_empty()
        || !artifact.dynamic_conformance_tables.is_empty()
        || !artifact.forwarded_dynamic_descriptor_adapters.is_empty()
        || !artifact.forwarded_dynamic_descriptor_tables.is_empty()
        || artifact.private_functions.len()
            != private_functions.map_or(artifact.private_functions.len(), |roster| roster.len())
        || !artifact.port_effects.is_empty()
        || layout.function_symbols.len() != artifact.private_functions.len()
        || layout.sections.len() != 1
        || layout.symbols.len()
            != text.functions.len()
                + artifact.private_functions.len()
                + layout.normalized_imports.len()
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
        || section.size != artifact.text_bytes.len()
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
            .ok_or(Error::Mismatch("missing current fragment"))?;
        let (attachment, provenance) = source::fragment_metadata(source, placed.machine)?;
        let offset = host(placed.section_offset)?;
        let length = host(placed.byte_count)?;
        let entry = placed.machine == text.semantic_entry;
        let expected_name = if entry {
            entry_symbol_name(text.target)
        } else {
            format!("omega_terminal_machine_{}", placed.machine.get())
        };
        let unit_abi_matches = match (&function.parameter_abi, source::parameter_abi(targeted)) {
            (None, None) => true,
            (Some(actual), Some((call_plan, parameters, _))) => {
                actual.call_plan == *call_plan && actual.parameters == parameters
            }
            _ => false,
        };
        if function.machine != placed.machine
            || function.attachment != attachment
            || function.provenance != *provenance
            || function.scalar_abi != targeted.scalar_abi
            || function.mixed_structural_scalar_abi != targeted.mixed_structural_scalar_abi
            || !unit_abi_matches
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
            || !empty_unsupported_records(function)
        {
            return Err(Error::Mismatch(
                "shared object function or symbol differs from current data",
            ));
        }
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
        {
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
                &function.scalar_call_stacks,
            )?;
            let ordinary_rows =
                super::structural::validate_settlement_attributions(source, placed.machine, &rows)?;
            attribution::validate(fragment, abstracted, &ordinary_rows)?;
            super::structural::validate_function(source, function, &rows)?;
        }
    }
    if attribution_cursor != artifact.semantic_code_attribution.len() {
        return Err(Error::Mismatch(
            "shared object retains a foreign semantic attribution",
        ));
    }
    // Private carriers join by position: their symbol rows sit between the
    // program symbols and the import tail. The carrier/symbol/region join is
    // self-evident on the object; when the materialized roster is supplied
    // each row must additionally equal the record that produced it.
    let mut private_symbols = layout.symbols.iter().skip(text.functions.len());
    let mut private_offset = text.bytes.len();
    for ((index, carrier), (_, function_symbol)) in artifact
        .private_functions
        .iter()
        .enumerate()
        .zip(layout.function_symbols.iter())
    {
        let length = carrier.function.byte_count;
        let Some((symbol_handle, symbol)) = private_symbols.next() else {
            return Err(Error::Mismatch("object lacks a private function symbol"));
        };
        if carrier.function.symbol != symbol_handle
            || carrier.function.text_offset != private_offset
            || function_symbol.identity != carrier.identity
            || function_symbol.symbol != symbol_handle
            || symbol.section != SymbolSection::Section(SectionKind::Text)
            || symbol.offset != private_offset
            || symbol.size != length
            || symbol.kind != SymbolKind::Function
            || symbol.name.is_empty()
            || !symbol.import_library.is_empty()
            || artifact
                .text_bytes
                .get(private_offset..private_offset + length)
                .is_none()
        {
            return Err(Error::Mismatch(
                "private function carrier is inconsistent with the emitted object",
            ));
        }
        if let Some(private) = private_functions.and_then(|roster| roster.get(index))
            && (carrier.identity != private.identity
                || carrier.source_psi != private.source_psi
                || carrier.function.machine != private.function.machine
                || carrier.function.scalar_abi != private.function.scalar_abi
                || length != private.function.bytes.len()
                || symbol.name != private.private_symbol.as_ref()
                || artifact.text_bytes[private_offset..private_offset + length]
                    != private.function.bytes[..])
        {
            return Err(Error::Mismatch(
                "private function carrier differs from the materialized record",
            ));
        }
        private_offset = private_offset.checked_add(length).ok_or(Error::Overflow)?;
    }
    super::structural::validate_settlements(source, &artifact.boundary_settlements)?;
    super::imports::validate(source, artifact)?;
    if let Some(binding) = artifact.hosted_receiver_binding() {
        crate::hosted_receiver::validate_binding(artifact, binding)
            .map_err(|_| Error::Mismatch("hosted receiver binding differs from replayed object"))?;
    }
    Ok(())
}

fn empty_unsupported_records(function: &ObjectFunction) -> bool {
    function.structural_call_scalar_return.is_none()
        && function.x86_scalar_fma.is_empty()
        && function.x86_scalar_fma_occurrences.is_empty()
        && function.x86_floating_control.is_none()
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
        && function.unit_continuations.is_empty()
        && function.scalar_affine_cleanup.is_none()
        && function.scalar_control_affine_cleanups.is_empty()
        && function.structural_return.is_none()
}
