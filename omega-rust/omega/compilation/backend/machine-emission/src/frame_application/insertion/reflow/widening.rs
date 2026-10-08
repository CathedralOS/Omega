//! Monotonic widening of short conditional branches that frame bytes moved
//! out of reach.
//!
//! Each round finds every short row whose taken edge left its form's reach
//! (`branch_reach`) and widens all of them at once: four bytes enter right
//! after each short row, and every later block, row, fixup and epilogue
//! coordinate moves by the growth inserted before it. Widening only lengthens
//! rows, so a row out of reach stays out of reach and the rounds reach the
//! least widening closure; each round retires at least one short row.

use std::collections::BTreeMap;

use machine_code::{
    FunctionAppliedFrameEpilogue, FunctionFragment, FunctionFragmentBranchEvidence,
};
use target::Architecture;

use super::super::branch_reach::{WIDENING_GROWTH, short_conditional_form};
use super::super::compute::shift_row_fixups;
use super::FrameApplicationError;

pub(super) fn widen_out_of_range_branches(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    architecture: Architecture,
) -> Result<(), FrameApplicationError> {
    loop {
        let rows = out_of_reach_short_rows(function, architecture)?;
        if rows.is_empty() {
            return Ok(());
        }
        widen(function, epilogues, &rows)?;
    }
}

/// `(block index, row index)` of every short conditional row whose taken edge
/// lies outside its form's reach, in layout order.
fn out_of_reach_short_rows(
    function: &FunctionFragment,
    architecture: Architecture,
) -> Result<Vec<(usize, usize)>, FrameApplicationError> {
    let block_offsets = function
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (row_index, row) in block.instructions.iter().enumerate() {
            let Some(FunctionFragmentBranchEvidence::Conditional(branch)) = row.branch.as_deref()
            else {
                continue;
            };
            let Some(form) = short_conditional_form(architecture, branch) else {
                continue;
            };
            if u64::try_from(row.bytes.len()).ok() != Some(form.width) {
                continue;
            }
            let target = block_offsets.get(&branch.when_taken_block).copied().ok_or(
                FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
            )?;
            if !form.reaches(row.offset, target) {
                rows.push((block_index, row_index));
            }
        }
    }
    Ok(rows)
}

/// Widen every listed row in one pass. The rows arrive in layout order, so
/// their insertion points ascend.
fn widen(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    rows: &[(usize, usize)],
) -> Result<(), FrameApplicationError> {
    let growth =
        usize::try_from(WIDENING_GROWTH).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let mut insertions = Vec::with_capacity(rows.len());
    for &(block_index, row_index) in rows {
        let block = &mut function.blocks[block_index];
        let row = &mut block.instructions[row_index];
        let short_width =
            u64::try_from(row.bytes.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
        insertions.push(
            row.offset
                .checked_add(short_width)
                .ok_or(FrameApplicationError::OffsetOverflow)?,
        );
        row.bytes.resize(row.bytes.len() + growth, 0);
        block.byte_count = grow(block.byte_count, 1)?;
    }
    if insertions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FrameApplicationError::ArtifactMismatch);
    }

    let mut bytes = Vec::with_capacity(function.bytes.len() + rows.len() * growth);
    let mut cursor = 0_usize;
    for &insertion in &insertions {
        let position =
            usize::try_from(insertion).map_err(|_| FrameApplicationError::OffsetOverflow)?;
        bytes.extend_from_slice(
            function
                .bytes
                .get(cursor..position)
                .ok_or(FrameApplicationError::ArtifactMismatch)?,
        );
        bytes.resize(bytes.len() + growth, 0);
        cursor = position;
    }
    bytes.extend_from_slice(&function.bytes[cursor..]);
    function.bytes = bytes;
    function.byte_count = grow(function.byte_count, insertions.len())?;

    // A coordinate moves by the growth inserted at or before it; the widened
    // row itself starts before its own insertion point.
    let shift = |offset: u64| {
        grow(
            offset,
            insertions.partition_point(|&insertion| insertion <= offset),
        )
    };
    for block in &mut function.blocks {
        block.offset = shift(block.offset)?;
        for row in &mut block.instructions {
            let shifted = shift(row.offset)?;
            let delta = shifted - row.offset;
            row.offset = shifted;
            if delta != 0 {
                shift_row_fixups(row, delta)?;
            }
        }
    }
    for epilogue in epilogues {
        epilogue.function_offset = shift(epilogue.function_offset)?;
    }
    Ok(())
}

/// `offset` plus `count` widenings.
fn grow(offset: u64, count: usize) -> Result<u64, FrameApplicationError> {
    u64::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(WIDENING_GROWTH))
        .and_then(|growth| offset.checked_add(growth))
        .ok_or(FrameApplicationError::OffsetOverflow)
}
