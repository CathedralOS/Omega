//! Producer-local exact five-axis usage accounting.

use optimization_core::OptimizationWorkUsage;

use crate::{FunctionRecursiveReloadValueHomes, RecursiveReloadValueHomeError};

pub(super) fn usage(
    functions: &[FunctionRecursiveReloadValueHomes],
) -> Result<OptimizationWorkUsage, RecursiveReloadValueHomeError> {
    let functions_count = to_u64(functions.len())?;
    let mut assignments = 0_u64;
    let mut candidates = 0_u64;
    let mut rosters = 0_u64;
    for row in functions.iter().flat_map(|function| &function.assignments) {
        assignments = checked(assignments, 1)?;
        candidates = checked(candidates, to_u64(row.candidates.len())?)?;
        rosters = checked(rosters, to_u64(row.coexisting_homes.len())?)?;
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: checked(functions_count, assignments)?,
        candidates,
        validation_steps: checked(
            checked(candidates, rosters)?,
            assignments
                .checked_mul(6)
                .ok_or(RecursiveReloadValueHomeError::WorkOverflow)?,
        )?,
        commits: assignments,
        iterations: checked(functions_count, assignments)?,
    })
}

fn checked(left: u64, right: u64) -> Result<u64, RecursiveReloadValueHomeError> {
    left.checked_add(right)
        .ok_or(RecursiveReloadValueHomeError::WorkOverflow)
}

fn to_u64(value: usize) -> Result<u64, RecursiveReloadValueHomeError> {
    u64::try_from(value).map_err(|_| RecursiveReloadValueHomeError::WorkOverflow)
}
