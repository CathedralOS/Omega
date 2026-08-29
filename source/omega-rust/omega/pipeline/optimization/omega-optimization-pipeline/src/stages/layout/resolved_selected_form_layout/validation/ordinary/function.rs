use std::collections::BTreeMap;

use omega_machine_optimizer::PostAllocationMachineFunction;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::SelectedFunction;
use omega_target::Architecture;

use super::super::row;
use super::{Fusion, PreLayoutRows, order, plan, roster};

use super::super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedSelectedFunctionLayout,
};

pub(super) fn validate(
    architecture: Architecture,
    selected: &SelectedFunction,
    machine: &PostAllocationMachineFunction,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Fusion<'_>,
    pre_rows: &mut PreLayoutRows<'_>,
    candidate: &ResolvedSelectedFunctionLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    if selected.machine != machine.machine
        || selected.machine != candidate.machine
        || selected.blocks.len() != machine.blocks.len()
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let pre_rows = roster::collect_pre_layout_rows(selected, pre_rows)?;
    let machine_blocks = machine
        .blocks
        .iter()
        .map(|block| (block.block, block))
        .collect::<BTreeMap<_, _>>();
    if machine_blocks.len() != machine.blocks.len() {
        return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
    }
    let ordered = order::derive(selected, fusion)?;
    if candidate.blocks.len() != ordered.len() {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    let layout = plan::derive(architecture, &ordered, &pre_rows)?;
    if candidate.byte_count != layout.function_size {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    for (block, candidate_block) in ordered.iter().copied().zip(&candidate.blocks) {
        let machine_block = machine_blocks
            .get(&block.id)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::RootMismatch)?;
        let instructions = roster::instructions(block);
        if machine_block.instructions.len() != instructions.len()
            || candidate_block.instructions.len() != instructions.len()
            || candidate_block.block != block.id
            || candidate_block.offset != layout.block_offsets[&block.id]
            || candidate_block.byte_count != layout.block_sizes[&block.id]
        {
            return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
        }
        let mut instruction_offset = candidate_block.offset;
        for ((instruction, machine_instruction), candidate_row) in instructions
            .into_iter()
            .zip(&machine_block.instructions)
            .zip(&candidate_block.instructions)
        {
            if machine_instruction.instruction != instruction.id {
                return Err(OptimizedResolvedSelectedFormLayoutError::RootMismatch);
            }
            let pre = pre_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            row::validate(
                architecture,
                selected.machine,
                block,
                instruction,
                machine_instruction,
                pre,
                physical,
                fusion,
                instruction_offset,
                &layout.block_offsets,
                candidate_row,
            )?;
            instruction_offset = instruction_offset
                .checked_add(
                    u64::try_from(candidate_row.bytes.len())
                        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?,
                )
                .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?;
        }
        if instruction_offset
            != candidate_block
                .offset
                .checked_add(candidate_block.byte_count)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)?
        {
            return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
        }
    }
    Ok(())
}
