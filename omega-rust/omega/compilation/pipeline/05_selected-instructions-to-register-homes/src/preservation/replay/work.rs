use optimization_core::OptimizationWorkUsage;

use super::{super::AllocatedCalleeSavedRequirementError, state::ReplayTraversal};

pub(super) fn usage(
    traversal: &ReplayTraversal<'_>,
) -> Result<OptimizationWorkUsage, AllocatedCalleeSavedRequirementError> {
    let modified_units = traversal
        .functions
        .iter()
        .try_fold(0_u64, |total, function| {
            add(total, count(function.modified_units.len())?)
        })?;
    let aggregate = [
        traversal.function_count,
        traversal.block_count,
        traversal.instruction_count,
        traversal.operand_count,
        traversal.write_count,
    ]
    .into_iter()
    .try_fold(0_u64, add)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(traversal.function_count, 1)?,
        candidates: traversal.write_count,
        validation_steps: aggregate,
        commits: [
            1,
            traversal.function_count,
            modified_units,
            traversal.witness_count,
        ]
        .into_iter()
        .try_fold(0_u64, add)?,
        iterations: aggregate,
    })
}

fn count(value: usize) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    u64::try_from(value).map_err(|_| AllocatedCalleeSavedRequirementError::WorkOverflow)
}

fn add(value: u64, increment: u64) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    value
        .checked_add(increment)
        .ok_or(AllocatedCalleeSavedRequirementError::WorkOverflow)
}
