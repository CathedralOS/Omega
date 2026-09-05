//! Ordinary fragments preserve layout order and every selected provenance fact.

use super::{ResolvedFragmentEmissionError, byte_span, instruction, require};
use machine_code::{FunctionFragmentEmissionPlan, ResolvedMachineLayout};
use selected_instructions::{SelectedInstructionPlan, SelectedTerminator};

pub(super) fn check(
    selected: &SelectedInstructionPlan,
    layout: &ResolvedMachineLayout,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        selected.functions.len() == layout.functions.len()
            && fragments.functions.len() == selected.functions.len(),
    )?;
    for (source, claimed) in selected.functions.iter().zip(&fragments.functions) {
        let resolved = layout
            .functions
            .iter()
            .find(|row| row.machine == source.machine)
            .ok_or(ResolvedFragmentEmissionError::MissingFunction(
                source.machine,
            ))?;
        require(
            claimed.machine == source.machine
                && claimed.attachment == source.attachment
                && claimed.provenance == source.provenance
                && claimed.byte_count == resolved.byte_count
                && u64::try_from(claimed.bytes.len()).ok() == Some(claimed.byte_count)
                && claimed.blocks.len() == resolved.blocks.len()
                && resolved.blocks.len() == source.blocks.len(),
        )?;
        let mut offset = 0_u64;
        for (block, actual) in resolved.blocks.iter().zip(&claimed.blocks) {
            let selected_block = source
                .blocks
                .iter()
                .find(|row| row.id == block.block)
                .ok_or(ResolvedFragmentEmissionError::MissingBlock(block.block))?;
            require(
                actual.block == block.block
                    && actual.offset == block.offset
                    && block.offset == offset
                    && actual.byte_count == block.byte_count
                    && actual.instructions.len() == block.instructions.len()
                    && block.instructions.len() == selected_block.instructions.len() + 1,
            )?;
            for (row, span) in block.instructions.iter().zip(&actual.instructions) {
                let terminal = match &selected_block.terminator {
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
                    | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction,
                };
                let selected_instruction = selected_block
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminal))
                    .find(|value| value.id == row.instruction)
                    .ok_or(ResolvedFragmentEmissionError::MissingInstruction(
                        row.instruction,
                    ))?;
                require(row.offset == offset)?;
                instruction::check(selected_block, selected_instruction, row, span)?;
                byte_span(&claimed.bytes, offset, &span.bytes)?;
                offset = offset
                    .checked_add(
                        u64::try_from(span.bytes.len())
                            .map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?,
                    )
                    .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?;
            }
            require(offset.checked_sub(block.offset) == Some(block.byte_count))?;
        }
        require(offset == claimed.byte_count)?;
    }
    Ok(())
}
