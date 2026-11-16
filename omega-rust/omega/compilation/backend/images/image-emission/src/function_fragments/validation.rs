//! Independently replay object custody, bytes, geometry and settlements.
//!
//! Every read checks the object against its current producer chain: the
//! resolved machine program, its contracts and the facts the driver carried.
//! Comparing the object with what stage 02 lowered needs stage 02's output,
//! which only a compile that requested a correspondence audit keeps; without
//! it those comparisons do not run, and the mandatory current checks still do
//! (wiki/spec/build/command_line.md "Independent stage validators").

mod stack {
    use super::super::source::CallStackEdge;
    use super::{Error, host};
    use crate::{ObjectScalarCallStack, ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack};
    use target::Architecture;

    pub(super) fn validate(
        unit: bool,
        geometry: (u64, u16, bool),
        architecture: Architecture,
        temporary_bytes: u32,
        calls: &[CallStackEdge],
        unit_stack: Option<ObjectUnitStack>,
        scalar_stack: Option<ObjectScalarStack>,
        unit_call_stacks: &[ObjectUnitCallStack],
        scalar_call_stacks: &[ObjectScalarCallStack],
    ) -> Result<(), Error> {
        let (frame_bytes, alignment, contains_call) = geometry;
        if (unit && (!scalar_call_stacks.is_empty() || unit_call_stacks.len() != calls.len()))
            || (!unit && (!unit_call_stacks.is_empty() || scalar_call_stacks.len() != calls.len()))
            || alignment == 0
        {
            return Err(Error::Mismatch("shared call stack roster changed"));
        }
        let return_bytes = match architecture {
            Architecture::X86_64 => 8u64,
            Architecture::Aarch64 => 0,
        };
        let mut peak = frame_bytes
            .checked_add(u64::from(temporary_bytes))
            .ok_or(Error::Overflow)?;
        let edges = unit_call_stacks
            .iter()
            .map(|row| {
                (
                    row.owner,
                    row.target,
                    row.text_offset,
                    row.caller_live_bytes,
                )
            })
            .chain(scalar_call_stacks.iter().map(|row| {
                (
                    row.owner,
                    row.target,
                    row.text_offset,
                    row.caller_live_bytes,
                )
            }));
        for ((owner, target, text_offset, caller_live_bytes), call) in edges.zip(calls) {
            if !contains_call
                || owner != call.owner
                || target != call.callee
                || text_offset != host(call.section_offset)?
                || u64::from(caller_live_bytes)
                    != frame_bytes
                        .checked_add(return_bytes)
                        .ok_or(Error::Overflow)?
                || !caller_live_bytes.is_multiple_of(u32::from(alignment))
            {
                return Err(Error::Mismatch(
                    "shared call prefix differs from its validated frame and call",
                ));
            }
            peak = peak.max(u64::from(caller_live_bytes));
        }
        if unit_call_stacks.iter().any(|row| {
            u64::from(row.active_frame_bytes) != frame_bytes
                || u64::from(row.transient_bytes) != return_bytes
        }) {
            return Err(Error::Mismatch("shared call frame decomposition changed"));
        }
        match (unit, unit_stack, scalar_stack) {
            (true, Some(stack), None)
                if u64::from(stack.frame_bytes) == frame_bytes
                    && u64::from(stack.local_peak_bytes) == peak
                    && stack.stack_alignment == u32::from(alignment) =>
            {
                Ok(())
            }
            (false, None, Some(stack))
                if u64::from(stack.local_peak_bytes) == peak
                    && stack.stack_alignment == u32::from(alignment) =>
            {
                Ok(())
            }
            _ => Err(Error::Mismatch("shared function stack envelope changed")),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Architecture, ObjectScalarCallStack, ObjectScalarStack, ObjectUnitCallStack,
            ObjectUnitStack, validate,
        };
        use machine_code::{
            InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
            PlacedInternalMachineCallResolution,
        };
        use selected_instructions::{SelectedBlockId, SelectedInstructionId};
        use semantic_vocabulary::{MachineId, OperationId};

        fn call(architecture: Architecture) -> PlacedInternalMachineCallResolution {
            let x86 = architecture == Architecture::X86_64;
            let opcode = 12;
            let field = opcode + u64::from(x86);
            let next = opcode + if x86 { 5 } else { 4 };
            PlacedInternalMachineCallResolution {
                kind: if x86 {
                    InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1
                } else {
                    InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1
                },
                state: InternalMachineCallResolutionState::ResolvedInSectionV1,
                caller: MachineId::new(1).unwrap(),
                callee: MachineId::new(2).unwrap(),
                block: SelectedBlockId(0),
                instruction: SelectedInstructionId(3),
                owner: target_operations::CallSiteOwner::Operation(OperationId::new(3).unwrap()),
                call_function_offset: opcode,
                call_section_offset: opcode,
                call_byte_count: next - opcode,
                opcode_function_offset: opcode,
                opcode_section_offset: opcode,
                field_function_offset: field,
                field_section_offset: field,
                next_instruction_function_offset: next,
                next_instruction_section_offset: next,
                callee_section_offset: 64,
                field_byte_width: 4,
                addend: 0,
                displacement: if x86 { 47 } else { 13 },
            }
        }

        #[test]
        fn balanced_private_storage_is_included_in_peak_without_a_call() {
            let stack = ObjectScalarStack {
                local_peak_bytes: 40,
                stack_alignment: 16,
            };
            let check = |stack| {
                validate(
                    false,
                    (24, 16, false),
                    Architecture::X86_64,
                    16,
                    &[],
                    None,
                    Some(stack),
                    &[],
                    &[],
                )
            };
            check(stack).unwrap();
            let mut understated = stack;
            understated.local_peak_bytes = 24;
            assert!(check(understated).is_err());
            let mut overstated = stack;
            overstated.local_peak_bytes += 16;
            assert!(check(overstated).is_err());
        }

        #[test]
        fn scalar_call_edges_retain_exact_frame_prefix_owner_target_and_role() {
            for architecture in [Architecture::X86_64, Architecture::Aarch64] {
                let call = call(architecture);
                let (frame_bytes, peak) = match architecture {
                    Architecture::X86_64 => (24, 32),
                    Architecture::Aarch64 => (16, 16),
                };
                let stack = ObjectScalarStack {
                    local_peak_bytes: peak,
                    stack_alignment: 16,
                };
                let row = ObjectScalarCallStack {
                    owner: call.owner,
                    target: call.callee,
                    text_offset: call.field_section_offset as usize,
                    caller_live_bytes: peak,
                };
                let check = |stack, rows: &[ObjectScalarCallStack]| {
                    validate(
                        false,
                        (frame_bytes, 16, true),
                        architecture,
                        0,
                        &[(&call).into()],
                        None,
                        Some(stack),
                        &[],
                        rows,
                    )
                };
                check(stack, &[row]).unwrap();
                let mut changed = row;
                changed.caller_live_bytes += 16;
                assert!(check(stack, &[changed]).is_err());
                changed = row;
                changed.target = call.caller;
                assert!(check(stack, &[changed]).is_err());
                changed = row;
                changed.owner =
                    target_operations::CallSiteOwner::Operation(OperationId::new(9).unwrap());
                assert!(check(stack, &[changed]).is_err());
                changed = row;
                changed.text_offset += 1;
                assert!(check(stack, &[changed]).is_err());
                let mut changed_stack = stack;
                changed_stack.local_peak_bytes += 16;
                assert!(check(changed_stack, &[row]).is_err());
                assert!(check(stack, &[]).is_err());
                assert!(check(stack, &[row, row]).is_err());
                assert!(
                    validate(
                        true,
                        (frame_bytes, 16, true),
                        architecture,
                        0,
                        &[(&call).into()],
                        Some(ObjectUnitStack {
                            frame_bytes: frame_bytes as u32,
                            local_peak_bytes: peak,
                            stack_alignment: 16
                        }),
                        None,
                        &[],
                        &[row]
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn retained_call_prefix_and_local_peak_cannot_be_changed_or_removed() {
            for architecture in [Architecture::X86_64, Architecture::Aarch64] {
                let call = call(architecture);
                let (frame_bytes, transient_bytes, peak) = match architecture {
                    Architecture::X86_64 => (24, 8, 32),
                    Architecture::Aarch64 => (16, 0, 16),
                };
                let stack = ObjectUnitStack {
                    frame_bytes,
                    local_peak_bytes: peak,
                    stack_alignment: 16,
                };
                let row = ObjectUnitCallStack {
                    owner: call.owner,
                    target: call.callee,
                    text_offset: call.field_section_offset as usize,
                    active_frame_bytes: frame_bytes,
                    transient_bytes,
                    caller_live_bytes: peak,
                };
                let check = |stack, rows: &[ObjectUnitCallStack]| {
                    validate(
                        true,
                        (u64::from(frame_bytes), 16, true),
                        architecture,
                        0,
                        &[(&call).into()],
                        Some(stack),
                        None,
                        rows,
                        &[],
                    )
                };
                check(stack, &[row]).unwrap();
                let mut prefix = row;
                prefix.active_frame_bytes += 16;
                prefix.caller_live_bytes += 16;
                assert!(check(stack, &[prefix]).is_err());
                let mut transient = row;
                transient.transient_bytes += 16;
                transient.caller_live_bytes += 16;
                assert!(check(stack, &[transient]).is_err());
                let mut peak = stack;
                peak.local_peak_bytes += 16;
                assert!(check(peak, &[row]).is_err());
                assert!(check(stack, &[]).is_err());
            }
        }
    }
}

use super::{Error, attribution, host, source};
use crate::{ObjectArtifact, ObjectFunction};
use object_file::{
    SectionKind, StagedOptimizedRelocationFreeObjectContainer, SymbolKind, SymbolSection,
    entry_symbol_name,
};

pub(crate) fn validate_function_fragment_object_artifact(
    replay: &super::replay::FragmentReplay,
    artifact: &ObjectArtifact,
) -> Result<(), Error> {
    let source = replay.0.as_ref();
    crate::entries::physical_entry::validate_binding(artifact).map_err(Error::PhysicalEntry)?;
    if artifact.composition_source.is_some() {
        let root = artifact
            .composition_source
            .as_ref()
            .and_then(|root| root.fragment_replay.as_ref())
            .ok_or(Error::Mismatch("composition root lacks fragment custody"))?;
        if root.0.custody() != source.custody() {
            return Err(Error::Mismatch("composition source custody changed"));
        }
        crate::object::private_object_composition::validate_composition(artifact)?;
        if let Some(binding) = artifact.hosted_receiver_binding() {
            crate::entries::hosted_receiver::validate_binding(artifact, binding)
                .map_err(|_| Error::Mismatch("composed hosted receiver binding differs"))?;
        }
        return Ok(());
    }
    validate(source, replay.1.as_ref(), artifact)
}

fn validate(
    source: &StagedOptimizedRelocationFreeObjectContainer,
    original: Option<&crate::RetainedTargetOperations>,
    artifact: &ObjectArtifact,
) -> Result<(), Error> {
    object_file::validate_optimized_relocation_free_object_container(source)
        .map_err(Error::Source)?;
    let current = source.source().source().source().source();
    source::privileged_termination::validate(
        current.installation_authority().installed_entry_execution(),
        current.physical_program(),
    )?;
    if let Some(original) = original {
        source::retained_source_validation::admit(source, original)?;
        source::retained_source_validation::establish_geometry(source, original)?;
    }
    if artifact.profile != source::profile(source) {
        return Err(Error::Mismatch("object deployment profile changed"));
    }
    let text = source.source().text_section();
    let layout = &artifact.object.layout;
    let code_text_len =
        artifact
            .private_functions
            .iter()
            .try_fold(text.bytes.len(), |total, carrier| {
                total
                    .checked_add(carrier.function.byte_count)
                    .ok_or(Error::Overflow)
            })?;
    super::byte_literals::validate(source, artifact, code_text_len)?;
    super::code_descriptors::validate(source, artifact)?;
    super::native_code_layouts::validate(source, artifact)?;
    super::image_residence::validate(source, artifact)?;
    let private_identities = super::private_addresses::identities(
        &artifact.foreign_calls,
        &text.unresolved_image_addresses,
        text.psi,
    )?;
    let private_address_records = super::private_addresses::validate(
        &private_identities,
        &text.unresolved_image_addresses,
        artifact,
    )?;
    let code_descriptor_count = artifact
        .code_descriptor_pool
        .as_ref()
        .map_or(0, |pool| pool.descriptors.len());
    let native_count = artifact
        .native_code_layout_pool
        .as_ref()
        .map_or(0, |pool| pool.layouts.len());
    let pool = artifact.byte_literal_pool.as_ref();
    let expected_text_len = code_text_len
        .checked_add(pool.map_or(0, |pool| pool.byte_count))
        .ok_or(Error::Overflow)?;
    let requires_graph_storage_replay =
        text.functions
            .iter()
            .try_fold(false, |required, placed| -> Result<bool, Error> {
                let abstracted = source::abstract_function(source, placed.machine)?;
                Ok(required
                    || source::requires_graph_storage_replay(abstracted.operations())
                    || super::structural::requires_call_graph_replay(source, placed.machine)?)
            })?;
    if artifact.requires_graph_storage_replay != requires_graph_storage_replay
        || artifact.psi != text.psi
        || artifact.target != text.target
        || artifact.entry != text.semantic_entry
        || artifact.object.target != text.target
        || artifact.text_bytes.len() != expected_text_len
        || !artifact.text_bytes.starts_with(&text.bytes)
        || artifact.relocations.target != text.target
        || artifact.port_effects != super::port_effects::produce(source)?
        || layout.function_symbols.len() != private_identities.len()
        || layout.sections.len() != 1 + usize::from(code_descriptor_count + native_count != 0)
        || layout.symbols.len()
            != text.functions.len()
                + private_identities.len()
                + usize::from(pool.is_some())
                + code_descriptor_count
                + native_count
                + usize::from(artifact.retained_entry_storage.is_valid())
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
        .find(|(_, section)| section.kind == SectionKind::Text)
        .ok_or(Error::Mismatch("object has no text section"))?;
    if section.kind != SectionKind::Text
        || section.size != artifact.text_bytes.len()
        || section.alignment != host(text.section_alignment)?
    {
        return Err(Error::Mismatch("shared object text geometry changed"));
    }
    let fragments = source::fragments(source);
    let temporary_stack_bytes = if text.functions.is_empty() {
        std::collections::BTreeMap::new()
    } else {
        source::temporary_stack_bytes_by_function(source)?
    };
    let mut attribution_cursor = 0usize;
    for ((placed, function), (symbol_handle, symbol)) in text
        .functions
        .iter()
        .zip(&artifact.functions)
        .zip(layout.symbols.iter())
    {
        let abstracted = source::abstract_function(source, placed.machine)?;
        let abi = source::function_abi(source, placed.machine)?;
        let fragment = fragments
            .function(placed.machine)
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
        let parameter_abi_matches = |published: Option<(
            &calling_conventions::CallPlan,
            &[target_operations::ScalarAbiValue],
            &[target_operations::TargetStructuralParameter],
        )>| match (&function.parameter_abi, published) {
            (None, None) => true,
            (Some(actual), Some((call_plan, parameters, _))) => {
                actual.call_plan == *call_plan && actual.parameters == parameters
            }
            _ => false,
        };
        // The current contract is what the object was built from; stage 02's
        // function, when retained, must agree with both.
        let original_abi_matches = match original {
            None => true,
            Some(original) => {
                let (_, targeted) = source::retained_source_validation::audited_function(
                    source,
                    original,
                    placed.machine,
                )?;
                function.scalar_abi == targeted.scalar_abi
                    && function.mixed_structural_scalar_abi == targeted.mixed_structural_scalar_abi
                    && parameter_abi_matches(source::parameter_abi(targeted))
            }
        };
        if function.machine != placed.machine
            || function.attachment != attachment
            || function.provenance != *provenance
            || function.scalar_abi != abi.scalar
            || function.mixed_structural_scalar_abi != abi.mixed_structural_scalar
            || !parameter_abi_matches(abi.parameter_abi())
            || !original_abi_matches
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
            let calls = source::stack_calls(
                source,
                placed.machine,
                source::callable_invocations(source)?,
            )?;
            stack::validate(
                unit,
                (
                    frame.map_or(0, |frame| frame.frame_size_bytes),
                    frame.map_or(16, |frame| frame.abi_stack_alignment_bytes),
                    frame.is_some_and(|frame| frame.contains_call),
                ),
                text.target.architecture,
                *temporary_stack_bytes
                    .get(&placed.machine)
                    .ok_or(Error::Mismatch(
                        "missing selected function for temporary stack replay",
                    ))?,
                &calls,
                function.unit_stack,
                function.scalar_stack,
                &function.unit_call_stacks,
                &function.scalar_call_stacks,
            )?;
            let ordinary_rows =
                super::structural::validate_settlement_attributions(source, placed.machine, &rows)?;
            attribution::validate_current(
                fragment,
                source::publication_function(source, placed.machine)?,
                &ordinary_rows,
            )?;
            super::structural::validate_function(source, function, &rows)?;
        }
    }
    if attribution_cursor != artifact.semantic_code_attribution.len() {
        return Err(Error::Mismatch(
            "shared object retains a foreign semantic attribution",
        ));
    }
    let mut private_symbols = layout.symbols.iter().skip(text.functions.len());
    let mut private_offset = text.bytes.len();
    for (carrier, (_, function_symbol)) in artifact
        .private_functions
        .iter()
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
        private_offset = private_offset.checked_add(length).ok_or(Error::Overflow)?;
    }
    super::structural::validate_settlements(source, &artifact.boundary_settlements)?;
    super::imports::validate(source, artifact, private_address_records)?;
    if let Some(binding) = artifact.hosted_receiver_binding() {
        crate::entries::hosted_receiver::validate_binding(artifact, binding)
            .map_err(|_| Error::Mismatch("hosted receiver binding differs from replayed object"))?;
    }
    Ok(())
}

fn empty_unsupported_records(function: &ObjectFunction) -> bool {
    function.structural_call_scalar_return.is_none()
        && function.x86_floating_control.is_none()
        && function.internal_unit_scalar_calls.is_empty()
        && function.installed_provider_unit_scalar_calls.is_empty()
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
