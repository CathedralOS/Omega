use machine_code::FunctionFragmentBlockSpan;
use selected_instructions::{SelectedFunction, terminator_instruction};

use machine_code::ResolvedSelectedBlockLayout;

use super::instruction;
use crate::fragment_emission::projection::ResolvedFragmentEmissionError;

pub(super) fn emit(
    selected: &SelectedFunction,
    resolved: &ResolvedSelectedBlockLayout,
    bytes: &mut Vec<u8>,
) -> Result<FunctionFragmentBlockSpan, ResolvedFragmentEmissionError> {
    let block_start =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if block_start != resolved.offset {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let selected_block = selected
        .block(resolved.block)
        .ok_or(ResolvedFragmentEmissionError::MissingBlock(resolved.block))?;
    let mut instructions = Vec::with_capacity(resolved.instructions.len());
    // Layout emits each block's selected instructions followed by its terminator;
    // branch relaxation changes bytes and offsets, not this order. Carry that
    // association forward instead of searching the block again for every row.
    let mut selected_instructions =
        selected_block
            .instructions
            .iter()
            .chain(std::iter::once(terminator_instruction(
                &selected_block.terminator,
            )));
    for row in &resolved.instructions {
        let selected_instruction = selected_instructions.next().ok_or(
            ResolvedFragmentEmissionError::MissingInstruction(row.instruction),
        )?;
        instructions.push(instruction::emit(
            selected,
            selected_block,
            selected_instruction,
            row,
            bytes,
        )?);
    }
    if selected_instructions.next().is_some() {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let block_end =
        u64::try_from(bytes.len()).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    if block_end.checked_sub(block_start) != Some(resolved.byte_count) {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    Ok(FunctionFragmentBlockSpan {
        block: resolved.block,
        offset: resolved.offset,
        byte_count: resolved.byte_count,
        instructions,
    })
}
