//! Independent least widening closure from source rows and frame insertions.

use super::super::branch_reach::{WIDENING_GROWTH, short_conditional_form};
use super::FrameApplicationError;
use machine_code::{
    FunctionFragment, FunctionFragmentBranchEvidence, FunctionFragmentControlProvenance,
};
use std::collections::BTreeMap;
use target::Architecture;

pub(super) fn validate_widths(
    source: &FunctionFragment,
    candidate: &FunctionFragment,
    prologue: usize,
    epilogue: usize,
    architecture: Architecture,
) -> Result<(), FrameApplicationError> {
    let mut widths = source
        .blocks
        .iter()
        .map(|block| {
            block
                .instructions
                .iter()
                .map(|row| row.bytes.len())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let short_count = source
        .instructions()
        .filter(|row| match row.branch.as_deref() {
            Some(FunctionFragmentBranchEvidence::Conditional(branch)) => {
                short_conditional_form(architecture, branch)
                    .is_some_and(|form| row.bytes.len() as u64 == form.width)
            }
            _ => false,
        })
        .count();
    for _ in 0..=short_count {
        let mut cursor = prologue as u64;
        let mut blocks = BTreeMap::new();
        let mut offsets = Vec::new();
        for (block, row_widths) in source.blocks.iter().zip(&widths) {
            if blocks.insert(block.block, cursor).is_some() {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            let mut rows = Vec::new();
            for (row, width) in block.instructions.iter().zip(row_widths) {
                if matches!(
                    row.control,
                    FunctionFragmentControlProvenance::Return { .. }
                ) {
                    cursor = cursor
                        .checked_add(epilogue as u64)
                        .ok_or(FrameApplicationError::OffsetOverflow)?;
                }
                rows.push(cursor);
                cursor = cursor
                    .checked_add(*width as u64)
                    .ok_or(FrameApplicationError::OffsetOverflow)?;
            }
            offsets.push(rows);
        }
        let mut changed = false;
        for (block_index, block) in source.blocks.iter().enumerate() {
            for (row_index, row) in block.instructions.iter().enumerate() {
                let Some(FunctionFragmentBranchEvidence::Conditional(branch)) =
                    row.branch.as_deref()
                else {
                    continue;
                };
                let Some(form) = short_conditional_form(architecture, branch) else {
                    continue;
                };
                if widths[block_index][row_index] as u64 != form.width {
                    continue;
                }
                let target = blocks.get(&branch.when_taken_block).ok_or(
                    FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
                )?;
                if !form.reaches(offsets[block_index][row_index], *target) {
                    widths[block_index][row_index] += WIDENING_GROWTH as usize;
                    changed = true;
                }
            }
        }
        if !changed {
            if source.blocks.len() != candidate.blocks.len()
                || widths.iter().zip(&candidate.blocks).any(|(widths, block)| {
                    widths.len() != block.instructions.len()
                        || widths
                            .iter()
                            .zip(&block.instructions)
                            .any(|(width, row)| *width != row.bytes.len())
                })
            {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            return Ok(());
        }
    }
    Err(FrameApplicationError::ArtifactMismatch)
}
