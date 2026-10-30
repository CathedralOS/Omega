//! Byte measure and work charging.

use machine_code::ResolvedSelectedFunctionLayout;

use super::super::error::{OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis};

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

pub(super) fn checked_delta(
    target: u64,
    base: u64,
) -> Result<i64, OptimizedX86BranchRelaxationError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)
}

/// The offset a row of `bytes` bytes ends at. Every reflow site advances an
/// offset by a row width, so the checked add and its narrowing live here once.
pub(super) fn advance(offset: u64, bytes: usize) -> Result<u64, OptimizedX86BranchRelaxationError> {
    let width =
        u64::try_from(bytes).map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    offset
        .checked_add(width)
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)
}
