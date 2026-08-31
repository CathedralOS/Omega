//! Ordered branch locations, byte measure, and distinct producer/replay charging.

use crate::ResolvedSelectedFunctionLayout;

use super::super::error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis};

pub(super) fn ordered_branch_locations(
    functions: &[ResolvedSelectedFunctionLayout],
) -> Vec<(usize, usize, usize)> {
    let mut locations = Vec::new();
    for (function_index, function) in functions.iter().enumerate() {
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, row) in block.instructions.iter().enumerate() {
                if row.branch.is_some() {
                    locations.push((function_index, block_index, instruction_index));
                }
            }
        }
    }
    locations
}

pub(super) fn total_bytes(
    functions: &[ResolvedSelectedFunctionLayout],
) -> Result<u64, OptimizedX86BranchRelaxationError> {
    functions.iter().try_fold(0_u64, |total, function| {
        total
            .checked_add(function.byte_count)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)
    })
}

pub(super) fn charge(
    usage: &mut u64,
    limit: u64,
    axis: X86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if *usage > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    Ok(())
}

pub(super) fn replay_charge(
    usage: &mut u64,
    limit: u64,
    axis: X86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let next = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if next > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    *usage = next;
    Ok(())
}

pub(super) fn checked_delta(
    target: u64,
    base: u64,
) -> Result<i64, OptimizedX86BranchRelaxationError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)
}
