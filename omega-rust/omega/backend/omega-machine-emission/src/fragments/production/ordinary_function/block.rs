use omega_machine_code::FunctionFragmentBlockSpan;
use omega_selected_instructions::SelectedFunction;

use omega_machine_code::ResolvedSelectedBlockLayout;

use super::instruction;
use crate::fragments::ResolvedFragmentEmissionError;

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
        .blocks
        .iter()
        .find(|block| block.id == resolved.block)
        .ok_or(ResolvedFragmentEmissionError::MissingBlock(resolved.block))?;
    let mut instructions = Vec::with_capacity(resolved.instructions.len());
    for row in &resolved.instructions {
        instructions.push(instruction::emit(selected_block, row, bytes)?);
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
