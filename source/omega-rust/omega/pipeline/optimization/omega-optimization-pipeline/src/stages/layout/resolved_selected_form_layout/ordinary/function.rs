use std::collections::BTreeMap;

use omega_machine_optimizer::PostAllocationMachineInstruction;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedFunction, SelectedInstructionId};
use omega_target::Architecture;

use crate::{
    SelectedFormEncodingRow, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64SameViewCopyElision,
};

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedSelectedBlockLayout, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout,
};
use super::{order, plan, row};

pub(in super::super) fn layout(
    architecture: Architecture,
    function: &SelectedFunction,
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    machine_rows: &BTreeMap<SelectedInstructionId, &PostAllocationMachineInstruction>,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
    copy_elision: Option<&StagedOptimizedAarch64SameViewCopyElision>,
) -> Result<ResolvedSelectedFunctionLayout, OptimizedResolvedSelectedFormLayoutError> {
    let ordered = order::derive(function, fusion)?;
    let layout = plan::derive(architecture, &ordered, pre_rows)?;
    let mut blocks = Vec::with_capacity(ordered.len());
    for block in ordered {
        let block_offset = layout.block_offsets[&block.id];
        let mut instruction_offset = block_offset;
        let mut instructions = Vec::new();
        for instruction in plan::instructions(block) {
            let pre = pre_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            let machine = machine_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            if machine.alternative.key != pre.alternative {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::AlternativeMismatch(instruction.id),
                );
            }
            let (bytes, branch) = row::resolve(
                architecture,
                function.machine,
                block,
                instruction,
                instruction_offset,
                &layout.block_offsets,
                machine,
                pre,
                physical,
                fusion,
                copy_elision,
            )?;
            let byte_count = u64::try_from(bytes.len())
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
            instructions.push(ResolvedSelectedFormRow {
                instruction: instruction.id,
                alternative: pre.alternative,
                offset: instruction_offset,
                bytes,
                branch,
            });
            instruction_offset = instruction_offset
                .checked_add(byte_count)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        }
        let byte_count = layout.block_sizes[&block.id];
        if instruction_offset
            != block_offset
                .checked_add(byte_count)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?
        {
            return Err(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow);
        }
        blocks.push(ResolvedSelectedBlockLayout {
            block: block.id,
            offset: block_offset,
            byte_count,
            instructions,
        });
    }
    Ok(ResolvedSelectedFunctionLayout {
        machine: function.machine,
        byte_count: layout.function_size,
        blocks,
    })
}
