use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedFunction, SelectedInstructionId};
use target::Architecture;

use machine_code::SelectedFormEncodingRow;

use super::super::OptimizedResolvedSelectedFormLayoutError as Error;
use super::super::{
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    SelectedFunctionLayoutPolicy,
};
use super::{order, plan, row};

pub(in super::super) fn layout(
    architecture: Architecture,
    function: &SelectedFunction,
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    machine_rows: &BTreeMap<SelectedInstructionId, &PostAllocationMachineInstruction>,
    physical: &ValidatedPhysicalRegisterModel,
    policy: SelectedFunctionLayoutPolicy,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<ResolvedSelectedFunctionLayout, Error> {
    let ordered = order::derive(function, policy)?;
    let layout = plan::derive(architecture, &ordered, pre_rows)?;
    let mut blocks = Vec::with_capacity(ordered.len());
    for block in ordered {
        let block_offset = layout.block_offsets[&block.id];
        let mut instruction_offset = block_offset;
        let mut instructions = Vec::new();
        for instruction in plan::instructions(block) {
            let pre = pre_rows
                .get(&instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            let machine = machine_rows
                .get(&instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            if machine.alternative.key != pre.alternative {
                return Err(Error::AlternativeMismatch(instruction.id));
            }
            let (
                bytes,
                branch,
                internal_machine_fixup,
                normalized_foreign_call_fixup,
                image_address_fixup,
            ) = row::resolve(
                architecture,
                block,
                instruction,
                instruction_offset,
                &layout.block_offsets,
                machine,
                pre,
                physical,
                layout.widened_branches.contains(&instruction.id),
                encoded,
            )?;
            let byte_count = u64::try_from(bytes.len()).map_err(|_| Error::OffsetOverflow)?;
            instructions.push(ResolvedSelectedFormRow {
                instruction: instruction.id,
                alternative: pre.alternative,
                offset: instruction_offset,
                bytes,
                branch,
                internal_machine_fixup,
                normalized_foreign_call_fixup,
                image_address_fixup,
            });
            instruction_offset = instruction_offset
                .checked_add(byte_count)
                .ok_or(Error::OffsetOverflow)?;
        }
        let byte_count = layout.block_sizes[&block.id];
        if instruction_offset
            != block_offset
                .checked_add(byte_count)
                .ok_or(Error::OffsetOverflow)?
        {
            return Err(Error::OffsetOverflow);
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
