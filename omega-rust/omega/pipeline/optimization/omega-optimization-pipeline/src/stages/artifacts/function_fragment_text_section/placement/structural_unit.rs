mod fixup;
mod internal_call;
mod lookup;
mod offsets;

use omega_object_file::{
    PlacedBlockSpan, PlacedFunctionFragment, PlacedInstructionSpan,
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements,
};
use omega_optimization_core::TerminalRelocationFreeTextSectionIdentity;
use omega_target::Architecture;

use crate::{
    StagedOptimizedFunctionFragmentEmission, StagedOptimizedFunctionFragmentEmissionSource,
};

use super::super::RelocationFreeTextSectionPlacementError;
use super::conversion::{u64_to_usize, usize_to_u64};

pub(super) fn place(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let fragments = source.fragments();
    let StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(realization) =
        source.source()
    else {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    };
    if fragments.target.architecture != Architecture::X86_64 || !fragments.functions.is_empty() {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let selected_plan = source.source().selected_plan();
    let environment = source.source().register_environment();
    let machine_plan = realization.machine().machine().plan();
    let effects_plan = realization.machine().effects().effects().plan();
    let encoding = realization.encoding();
    let layout = realization.layout();
    let exit = realization.exit_contract().contract();
    let count = fragments.structural_unit_functions.len();
    if count == 0
        || selected_plan.structural_unit_functions.len() != count
        || machine_plan.structural_unit_functions.len() != count
        || effects_plan.structural_unit_functions.len() != count
        || encoding.structural_unit_functions().len() != count
        || layout.structural_unit_functions().len() != count
        || exit.structural_unit_functions.len() != count
        || !selected_plan.functions.is_empty()
        || !machine_plan.functions.is_empty()
        || !effects_plan.functions.is_empty()
        || !encoding.rows().is_empty()
        || !layout.functions().is_empty()
        || !exit.functions.is_empty()
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }

    let offsets = offsets::derive(&fragments.structural_unit_functions, fragments.entry)?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(offsets.section_byte_count)
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
    );
    let mut functions = Vec::with_capacity(count);
    let mut resolved_internal_machine_calls = Vec::new();
    for (source_function_index, fragment) in fragments.structural_unit_functions.iter().enumerate()
    {
        let function_section_offset = *offsets
            .by_machine
            .get(&fragment.machine)
            .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
        if usize_to_u64(bytes.len())? != function_section_offset {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let selected = lookup::unique_machine(
            &selected_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let machine = lookup::unique_machine(
            &machine_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let effects = lookup::unique_machine(
            &effects_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let encoded = lookup::unique_machine(
            encoding.structural_unit_functions(),
            fragment.machine,
            |function| function.machine,
        )?;
        let laid_out = lookup::unique_machine(
            layout.structural_unit_functions(),
            fragment.machine,
            |function| function.machine,
        )?;
        let exited = lookup::unique_machine(
            exit.structural_unit_functions.as_slice(),
            fragment.machine,
            |function| function.machine,
        )?;
        if fragment.block.block != selected.entry_block
            || fragment.block.block != machine.block
            || fragment.block.block != effects.block
            || fragment.block.block != encoded.block
            || fragment.block.block != laid_out.block
            || fragment.block.block != exited.returned.block
            || fragment.block.offset != 0
            || fragment.block.byte_count != fragment.byte_count
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }

        let mut function_bytes = fragment.bytes.clone();
        match (
            fragment.block.call.as_ref(),
            selected.call.as_ref(),
            machine.call.as_ref(),
            effects.call.as_ref(),
            encoded.call.as_ref(),
            laid_out.call.as_ref(),
            exited.call.as_ref(),
        ) {
            (None, None, None, None, None, None, None) => {}
            (
                Some(fragment_call),
                Some(selected_call),
                Some(machine_call),
                Some(effect_call),
                Some(encoded_call),
                Some(layout_call),
                Some(exit_call),
            ) => {
                resolved_internal_machine_calls.push(internal_call::resolve(
                    fragment.machine,
                    fragment.block.block,
                    function_section_offset,
                    &offsets.by_machine,
                    &mut function_bytes,
                    fragment_call,
                    selected_call,
                    machine_call,
                    effect_call,
                    encoded_call,
                    layout_call,
                    exit_call,
                    selected_plan.target,
                    environment.physical(),
                    environment.constraints(),
                )?);
            }
            _ => return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
        }

        let returned = &fragment.block.return_instruction;
        if returned.instruction != selected.terminator.instruction.id
            || returned.instruction != machine.return_instruction.instruction
            || returned.instruction != effects.return_instruction.instruction
            || returned.instruction != encoded.return_instruction.instruction
            || returned.instruction != laid_out.return_instruction.instruction
            || returned.instruction != exited.returned.instruction
            || returned.offset != laid_out.return_instruction.offset
            || returned.offset != exited.returned.offset
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let returned_section_offset = function_section_offset
            .checked_add(returned.offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        let returned_start = u64_to_usize(returned.offset)?;
        let returned_end = returned_start
            .checked_add(returned.bytes.len())
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        if function_bytes.get(returned_start..returned_end) != Some(returned.bytes.as_slice()) {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        bytes.extend_from_slice(&function_bytes);
        functions.push(PlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: fragment.machine,
            section_offset: function_section_offset,
            byte_count: fragment.byte_count,
            blocks: vec![PlacedBlockSpan {
                block: fragment.block.block,
                function_offset: fragment.block.offset,
                section_offset: function_section_offset,
                byte_count: fragment.block.byte_count,
                instructions: vec![PlacedInstructionSpan {
                    instruction: returned.instruction,
                    alternative: returned.alternative,
                    function_offset: returned.offset,
                    section_offset: returned_section_offset,
                    byte_count: u64::try_from(returned.bytes.len())
                        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                }],
            }],
        });
    }
    if usize_to_u64(bytes.len())? != offsets.section_byte_count
        || resolved_internal_machine_calls.len()
            != usize::try_from(
                source
                    .manifest()
                    .record()
                    .statistics
                    .unresolved_internal_machine_fixups,
            )
            .map_err(|_| RelocationFreeTextSectionPlacementError::StatisticsOverflow)?
    {
        return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
    }
    let mut text_section = RelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset: offsets.semantic_entry,
        policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment: 1,
        byte_count: offsets.section_byte_count,
        bytes,
        functions,
        resolved_internal_machine_calls,
        relocation_requirements:
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}
