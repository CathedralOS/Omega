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
    let aggregate = extent(traversal)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: add(traversal.function_count, 1)?,
        // The derivation is one linear traversal: its writes are traversal
        // work, counted below; its candidates are the requirements it derives.
        candidates: modified_units,
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

/// The program extent the replay traverses: every function, block,
/// instruction, operand and unit write, each read once. The pass budget's
/// ceilings scale with it (`OptimizationWorkBudget::for_extent`).
pub(super) fn extent(
    traversal: &ReplayTraversal<'_>,
) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    [
        traversal.function_count,
        traversal.block_count,
        traversal.instruction_count,
        traversal.operand_count,
        traversal.write_count,
    ]
    .into_iter()
    .try_fold(0_u64, add)
}

fn count(value: usize) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    u64::try_from(value).map_err(|_| AllocatedCalleeSavedRequirementError::WorkOverflow)
}

fn add(value: u64, increment: u64) -> Result<u64, AllocatedCalleeSavedRequirementError> {
    value
        .checked_add(increment)
        .ok_or(AllocatedCalleeSavedRequirementError::WorkOverflow)
}
