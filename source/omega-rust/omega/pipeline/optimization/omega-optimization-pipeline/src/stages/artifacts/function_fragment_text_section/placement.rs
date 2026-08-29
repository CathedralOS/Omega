use std::collections::{BTreeMap, BTreeSet};

use omega_isa_x86_64::{
    resolve_x86_64_structural_unit_internal_call,
    validate_x86_64_selected_structural_unit_call_template,
    X86_64StructuralUnitInternalControlFixupKind, X86_64StructuralUnitInternalControlFixupState,
};
use omega_machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
};
use omega_object_file::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState, PlacedBlockSpan,
    PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    RelocationFreeTextSectionPlacement, TextSectionPlacementPolicy,
    TextSectionRelocationRequirements,
};
use omega_optimization_core::TerminalRelocationFreeTextSectionIdentity;
use omega_selected_instructions::{MachineAlternativeFamily, MachineEncodedControlEffect};
use omega_target::Architecture;
use psi_core::MachineId;

use crate::{
    FunctionFragmentEmissionSourceKind, FunctionFragmentEmissionStage,
    StagedOptimizedFunctionFragmentEmission, StagedOptimizedFunctionFragmentEmissionSource,
};

use super::RelocationFreeTextSectionPlacementError;

pub(super) fn place_fragments(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let fragments = source.fragments();
    let source_manifest = source.manifest().record();
    match (
        fragments.functions.is_empty(),
        fragments.structural_unit_functions.is_empty(),
        source_manifest.stage,
        source_manifest.source_kind,
    ) {
        (
            false,
            true,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1,
            FunctionFragmentEmissionSourceKind::X86Rel8V1
            | FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 { .. }
            | FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1
            | FunctionFragmentEmissionSourceKind::UnitBaselineV1,
        ) => place_relocation_free_fragments(fragments),
        (
            true,
            false,
            FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
            | FunctionFragmentEmissionStage::ValidatedFunctionFragmentsWithUnresolvedInternalMachineFixupsV1,
            FunctionFragmentEmissionSourceKind::StructuralUnitV1,
        ) => place_structural_unit_fragments(source),
        _ => Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    }
}

fn place_relocation_free_fragments(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let section_alignment = match fragments.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    let mut bytes = Vec::new();
    let mut functions = Vec::with_capacity(fragments.functions.len());
    let mut seen_machines = BTreeSet::new();
    let mut semantic_entry_offset = None;

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_machines.insert(function.machine) {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        let section_offset = usize_to_u64(bytes.len())?;
        if function.machine == fragments.entry
            && semantic_entry_offset.replace(section_offset).is_some()
        {
            return Err(
                RelocationFreeTextSectionPlacementError::DuplicateSemanticEntry(fragments.entry),
            );
        }
        validate_architecture_alignment(
            fragments.target.architecture,
            section_offset,
            function.byte_count,
        )?;
        prove_function_needs_no_relocations(function)?;
        let blocks = place_blocks(fragments.target.architecture, function, section_offset)?;
        let function_start = bytes.len();
        bytes.extend_from_slice(&function.bytes);
        if usize_to_u64(bytes.len().saturating_sub(function_start))? != function.byte_count {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        functions.push(PlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: function.machine,
            section_offset,
            byte_count: function.byte_count,
            blocks,
        });
    }

    let semantic_entry_offset = semantic_entry_offset
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;
    let byte_count = usize_to_u64(bytes.len())?;
    let mut text_section = RelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset,
        policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment,
        byte_count,
        bytes,
        functions,
        resolved_internal_machine_calls: Vec::new(),
        relocation_requirements:
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}

fn place_structural_unit_fragments(
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

    let mut function_offsets = BTreeMap::new();
    let mut section_byte_count = 0_u64;
    let mut semantic_entry_offset = None;
    for function in &fragments.structural_unit_functions {
        if u64::try_from(function.bytes.len())
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?
            != function.byte_count
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        if function_offsets
            .insert(function.machine, section_byte_count)
            .is_some()
        {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        if function.machine == fragments.entry
            && semantic_entry_offset.replace(section_byte_count).is_some()
        {
            return Err(
                RelocationFreeTextSectionPlacementError::DuplicateSemanticEntry(fragments.entry),
            );
        }
        section_byte_count = section_byte_count
            .checked_add(function.byte_count)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    }
    let semantic_entry_offset = semantic_entry_offset
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;

    let mut bytes = Vec::with_capacity(
        usize::try_from(section_byte_count)
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
    );
    let mut functions = Vec::with_capacity(count);
    let mut resolved_internal_machine_calls = Vec::new();
    for (source_function_index, fragment) in fragments.structural_unit_functions.iter().enumerate()
    {
        let function_section_offset = *function_offsets
            .get(&fragment.machine)
            .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
        if usize_to_u64(bytes.len())? != function_section_offset {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let selected = unique_machine(
            &selected_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let machine = unique_machine(
            &machine_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let effects = unique_machine(
            &effects_plan.structural_unit_functions,
            fragment.machine,
            |function| function.machine,
        )?;
        let encoded = unique_machine(
            encoding.structural_unit_functions(),
            fragment.machine,
            |function| function.machine,
        )?;
        let laid_out = unique_machine(
            layout.structural_unit_functions(),
            fragment.machine,
            |function| function.machine,
        )?;
        let exited = unique_machine(
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
                if fragment_call.instruction != selected_call.id
                    || fragment_call.instruction != machine_call.instruction
                    || fragment_call.instruction != effect_call.instruction
                    || fragment_call.instruction != encoded_call.instruction
                    || fragment_call.instruction != layout_call.instruction
                    || fragment_call.instruction != exit_call.instruction
                    || fragment_call.operation != selected_call.operation
                    || fragment_call.operation != machine_call.operation
                    || fragment_call.operation != effect_call.operation
                    || fragment_call.operation != encoded_call.operation
                    || fragment_call.operation != layout_call.operation
                    || fragment_call.operation != exit_call.operation
                    || fragment_call.callee != selected_call.callee
                    || fragment_call.callee != machine_call.callee
                    || fragment_call.callee != effect_call.callee
                    || fragment_call.callee != encoded_call.callee
                    || fragment_call.callee != layout_call.callee
                    || fragment_call.callee != exit_call.callee
                    || fragment_call.provenance != selected_call.provenance
                    || fragment_call.provenance != effect_call.provenance
                    || fragment_call.offset != layout_call.offset
                    || fragment_call.offset != exit_call.offset
                    || encoded_call.footprint.as_ref() != layout_call.footprint.as_ref()
                    || encoded_call.fixup != layout_call.fixup
                    || encoded_call.fixup != exit_call.fixup
                {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                let template = validate_x86_64_selected_structural_unit_call_template(
                    selected_plan.target,
                    environment.physical(),
                    environment.constraints(),
                    selected_call,
                    effect_call.declaration,
                    &fragment_call.bytes,
                )
                .map_err(|error| {
                    RelocationFreeTextSectionPlacementError::StructuralUnitCallTemplate(
                        fragment.machine,
                        error,
                    )
                })?;
                if template.bytes() != fragment_call.bytes
                    || template.footprint() != encoded_call.footprint.as_ref()
                    || !fragment_fixup_matches_target(fragment_call, template.fixup())?
                {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                let call_section_offset = function_section_offset
                    .checked_add(fragment_call.offset)
                    .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
                let callee_section_offset = *function_offsets.get(&fragment_call.callee).ok_or(
                    RelocationFreeTextSectionPlacementError::MissingInternalMachineTarget(
                        fragment_call.callee,
                    ),
                )?;
                let resolved = resolve_x86_64_structural_unit_internal_call(
                    &template,
                    template.fixup(),
                    call_section_offset,
                    callee_section_offset,
                )
                .map_err(|error| {
                    RelocationFreeTextSectionPlacementError::StructuralUnitCallResolution(
                        fragment.machine,
                        error,
                    )
                })?;
                let call_start = u64_to_usize(fragment_call.offset)?;
                let call_end = call_start
                    .checked_add(resolved.bytes().len())
                    .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
                if function_bytes.get(call_start..call_end) != Some(fragment_call.bytes.as_slice())
                {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                function_bytes
                    .get_mut(call_start..call_end)
                    .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?
                    .copy_from_slice(resolved.bytes());
                let resolution = resolved.resolution();
                let neutral = fragment_call.fixup;
                resolved_internal_machine_calls.push(PlacedInternalMachineCallResolution {
                    kind: InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                    state: InternalMachineCallResolutionState::ResolvedInSectionV1,
                    caller: fragment.machine,
                    block: fragment.block.block,
                    instruction: fragment_call.instruction,
                    operation: fragment_call.operation,
                    callee: fragment_call.callee,
                    call_function_offset: fragment_call.offset,
                    call_section_offset,
                    call_byte_count: u64::try_from(resolved.bytes().len())
                        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    opcode_function_offset: neutral.opcode_function_offset,
                    opcode_section_offset: function_section_offset
                        .checked_add(neutral.opcode_function_offset)
                        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    field_function_offset: neutral.field_function_offset,
                    field_section_offset: function_section_offset
                        .checked_add(neutral.field_function_offset)
                        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    next_instruction_function_offset: neutral.next_instruction_function_offset,
                    next_instruction_section_offset: resolution.next_instruction_section_offset,
                    callee_section_offset: resolution.callee_section_offset,
                    field_byte_width: neutral.field_byte_width,
                    addend: neutral.addend,
                    displacement: resolution.displacement,
                });
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
    if usize_to_u64(bytes.len())? != section_byte_count
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
        semantic_entry_offset,
        policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment: 1,
        byte_count: section_byte_count,
        bytes,
        functions,
        resolved_internal_machine_calls,
        relocation_requirements:
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    text_section.identity = text_section.recomputed_identity();
    Ok(text_section)
}

fn fragment_fixup_matches_target(
    call: &omega_machine_code::StructuralUnitCallFragmentSpan,
    target: omega_isa_x86_64::X86_64StructuralUnitInternalControlFixup,
) -> Result<bool, RelocationFreeTextSectionPlacementError> {
    let neutral = call.fixup;
    Ok(neutral.kind
        == FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1
        && neutral.state
            == FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        && target.kind
            == X86_64StructuralUnitInternalControlFixupKind::Relative32FromNextInstructionToInternalMachineV1
        && target.state == X86_64StructuralUnitInternalControlFixupState::UnresolvedZeroFieldV1
        && neutral.callee == target.callee
        && neutral.callee == call.callee
        && neutral.opcode_function_offset
            == call
                .offset
                .checked_add(u64::from(target.opcode_byte_offset))
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?
        && neutral.field_function_offset
            == call
                .offset
                .checked_add(u64::from(target.field_byte_offset))
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?
        && neutral.next_instruction_function_offset
            == call
                .offset
                .checked_add(u64::from(target.next_instruction_byte_offset))
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?
        && neutral.field_byte_width == target.field_byte_width
        && neutral.addend == target.addend)
}

fn unique_machine<T>(
    functions: &[T],
    machine: MachineId,
    identify: impl Fn(&T) -> MachineId,
) -> Result<&T, RelocationFreeTextSectionPlacementError> {
    let mut matches = functions
        .iter()
        .filter(|function| identify(function) == machine);
    let function = matches
        .next()
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    if matches.next().is_some() {
        return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
            machine,
        ));
    }
    Ok(function)
}

#[cfg(test)]
pub(crate) fn place_fragments_for_test(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_relocation_free_fragments(fragments)
}

#[cfg(test)]
pub(crate) fn place_structural_unit_fragments_for_test(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    place_structural_unit_fragments(source)
}

fn place_blocks(
    architecture: Architecture,
    function: &FunctionFragment,
    function_section_offset: u64,
) -> Result<Vec<PlacedBlockSpan>, RelocationFreeTextSectionPlacementError> {
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        validate_architecture_alignment(architecture, block.offset, block.byte_count)?;
        let section_offset = function_section_offset
            .checked_add(block.offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        if block
            .offset
            .checked_add(block.byte_count)
            .is_none_or(|end| end > function.byte_count)
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for row in &block.instructions {
            let byte_count = usize_to_u64(row.bytes.len())?;
            validate_architecture_alignment(architecture, row.offset, byte_count)?;
            let row_section_offset = function_section_offset
                .checked_add(row.offset)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            if row
                .offset
                .checked_add(byte_count)
                .is_none_or(|end| end > function.byte_count)
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            instructions.push(PlacedInstructionSpan {
                instruction: row.instruction,
                alternative: row.alternative,
                function_offset: row.offset,
                section_offset: row_section_offset,
                byte_count,
            });
        }
        blocks.push(PlacedBlockSpan {
            block: block.block,
            function_offset: block.offset,
            section_offset,
            byte_count: block.byte_count,
            instructions,
        });
    }
    Ok(blocks)
}

fn prove_function_needs_no_relocations(
    function: &FunctionFragment,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    for block in &function.blocks {
        for row in &block.instructions {
            match row.alternative.family {
                MachineAlternativeFamily::ConditionalBranchNonZero => {
                    let Some(branch) = row.branch.as_deref() else {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    };
                    let FunctionFragmentControlProvenance::ConditionalBranch {
                        when_nonzero,
                        when_zero,
                    } = &row.control
                    else {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    };
                    if branch.source_block != block.block
                        || branch.when_nonzero_edge != when_nonzero.psi_edge
                        || branch.when_nonzero_block != when_nonzero.block
                        || branch.when_zero_edge != when_zero.psi_edge
                        || branch.when_zero_block != when_zero.block
                        || branch.decoded_effects.control
                            != MachineEncodedControlEffect::ConditionalRelativeBranchV1
                        || target_block_offset(function, branch.when_nonzero_block)
                            != Some(branch.when_nonzero_offset)
                        || target_block_offset(function, branch.when_zero_block)
                            != Some(branch.when_zero_offset)
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
                MachineAlternativeFamily::ReturnI64 | MachineAlternativeFamily::ReturnUnit => {
                    if row.branch.is_some()
                        || !matches!(
                            row.control,
                            FunctionFragmentControlProvenance::Return { .. }
                        )
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
                MachineAlternativeFamily::CompareI64Zero
                | MachineAlternativeFamily::MaterializeI64
                | MachineAlternativeFamily::CopyI64
                | MachineAlternativeFamily::ExactAddI64
                | MachineAlternativeFamily::ExactAddI64Immediate
                | MachineAlternativeFamily::ExactSubtractI64
                | MachineAlternativeFamily::ExactSubtractI64Immediate => {
                    if row.branch.is_some()
                        || row.control != FunctionFragmentControlProvenance::None
                    {
                        return Err(
                            RelocationFreeTextSectionPlacementError::UnsupportedRelocationShape,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn target_block_offset(
    function: &FunctionFragment,
    target: omega_selected_instructions::SelectedBlockId,
) -> Option<u64> {
    function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
}

fn validate_architecture_alignment(
    architecture: Architecture,
    offset: u64,
    byte_count: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(4) || !byte_count.is_multiple_of(4))
    {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    Ok(())
}

pub(super) fn usize_to_u64(value: usize) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    u64::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

fn u64_to_usize(value: u64) -> Result<usize, RelocationFreeTextSectionPlacementError> {
    usize::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}
