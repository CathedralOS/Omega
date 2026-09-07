//! Monotonic repair of short branches after frame bytes have been inserted.

use machine_code::{
    FunctionAppliedFrameEpilogue, FunctionFragment, FunctionFragmentBranchEvidence,
    FunctionFragmentConditionalBranchPredicate,
};
use target::Architecture;

use super::FrameApplicationError;

pub(super) fn widen_out_of_range_branches(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    architecture: Architecture,
) -> Result<(), FrameApplicationError> {
    if architecture != Architecture::X86_64 {
        return Ok(());
    }
    let short_count = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| {
            row.bytes.len() == 2
                && matches!(row.branch.as_deref(),
            Some(FunctionFragmentBranchEvidence::Conditional(branch))
                if branch.predicate == FunctionFragmentConditionalBranchPredicate::NonZeroV1)
        })
        .count();
    for _ in 0..=short_count {
        let Some((block_index, row_index)) = next_out_of_range(function)? else {
            return Ok(());
        };
        widen(function, epilogues, block_index, row_index)?;
    }
    Err(FrameApplicationError::ArtifactMismatch)
}

fn next_out_of_range(
    function: &FunctionFragment,
) -> Result<Option<(usize, usize)>, FrameApplicationError> {
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (row_index, row) in block.instructions.iter().enumerate() {
            let Some(FunctionFragmentBranchEvidence::Conditional(branch)) = row.branch.as_deref()
            else {
                continue;
            };
            if row.bytes.len() != 2
                || branch.predicate != FunctionFragmentConditionalBranchPredicate::NonZeroV1
            {
                continue;
            }
            let target = function
                .blocks
                .iter()
                .find(|block| block.block == branch.when_taken_block)
                .ok_or(FrameApplicationError::MissingTargetBlock(
                    branch.when_taken_block,
                ))?
                .offset;
            let displacement = i128::from(target) - (i128::from(row.offset) + 2);
            if i8::try_from(displacement).is_err() {
                return Ok(Some((block_index, row_index)));
            }
        }
    }
    Ok(None)
}

fn widen(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    block_index: usize,
    row_index: usize,
) -> Result<(), FrameApplicationError> {
    let insertion = function.blocks[block_index].instructions[row_index]
        .offset
        .checked_add(2)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    let position = usize::try_from(insertion).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    if position > function.bytes.len() {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    function.bytes.splice(position..position, [0; 4]);
    function.blocks[block_index].instructions[row_index]
        .bytes
        .resize(6, 0);
    function.blocks[block_index].byte_count = grow(function.blocks[block_index].byte_count)?;
    function.byte_count = grow(function.byte_count)?;
    for block in &mut function.blocks {
        if block.offset >= insertion {
            block.offset = grow(block.offset)?;
        }
        for row in &mut block.instructions {
            if row.offset >= insertion {
                row.offset = grow(row.offset)?;
                if let Some(fixup) = &mut row.internal_machine_fixup {
                    fixup.opcode_function_offset = grow(fixup.opcode_function_offset)?;
                    fixup.patch_function_offset = grow(fixup.patch_function_offset)?;
                    fixup.reference_function_offset = grow(fixup.reference_function_offset)?;
                }
            }
        }
    }
    for epilogue in epilogues {
        if epilogue.function_offset >= insertion {
            epilogue.function_offset = grow(epilogue.function_offset)?;
        }
    }
    Ok(())
}

fn grow(offset: u64) -> Result<u64, FrameApplicationError> {
    offset
        .checked_add(4)
        .ok_or(FrameApplicationError::OffsetOverflow)
}
