//! Replay-local exact work reconstruction.

use crate::{FunctionRecursiveReloadValueHomes, RecursiveReloadValueHomeError};
use optimization_core::OptimizationWorkUsage;

pub(super) fn reconstruct(
    functions: &[FunctionRecursiveReloadValueHomes],
) -> Result<OptimizationWorkUsage, RecursiveReloadValueHomeError> {
    let function_count = cast(functions.len())?;
    let mut rows = 0_u64;
    let mut candidates = 0_u64;
    let mut retained = 0_u64;
    for row in functions
        .iter()
        .flat_map(|function| function.assignments.iter())
    {
        rows = add(rows, 1)?;
        candidates = add(candidates, cast(row.candidates.len())?)?;
        retained = add(retained, cast(row.coexisting_homes.len())?)?;
    }
    let comparisons = rows
        .checked_mul(6)
        .ok_or(RecursiveReloadValueHomeError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(function_count, rows)?,
        candidates,
        validation_steps: add(add(candidates, retained)?, comparisons)?,
        commits: rows,
        iterations: add(function_count, rows)?,
    })
}

fn add(left: u64, right: u64) -> Result<u64, RecursiveReloadValueHomeError> {
    left.checked_add(right)
        .ok_or(RecursiveReloadValueHomeError::WorkOverflow)
}
fn cast(value: usize) -> Result<u64, RecursiveReloadValueHomeError> {
    u64::try_from(value).map_err(|_| RecursiveReloadValueHomeError::WorkOverflow)
}
