//! Replay-local exact work reconstruction.

use omega_optimization_core::OptimizationWorkUsage;

use crate::{AbstractSpillAccessConstraintError, FunctionAbstractSpillAccessConstraints};

pub(super) fn reconstruct(
    functions: &[FunctionAbstractSpillAccessConstraints],
) -> Result<OptimizationWorkUsage, AbstractSpillAccessConstraintError> {
    let function_count = cast(functions.len())?;
    let mut placement_count = 0_u64;
    let mut dependency_count = 0_u64;
    let mut pair_count = 0_u64;
    for function in functions {
        placement_count = add(placement_count, function.placements.len())?;
        dependency_count = add(dependency_count, function.dependencies.len())?;
        for (index, left) in function.placements.iter().enumerate() {
            for right in &function.placements[index + 1..] {
                if left.block == right.block {
                    pair_count = pair_count
                        .checked_add(1)
                        .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?;
                }
            }
        }
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count
            .checked_add(placement_count)
            .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?,
        candidates: pair_count,
        validation_steps: placement_count
            .checked_add(dependency_count)
            .and_then(|value| value.checked_add(pair_count))
            .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?,
        commits: placement_count
            .checked_add(dependency_count)
            .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?,
        iterations: function_count
            .checked_add(placement_count)
            .and_then(|value| value.checked_add(pair_count))
            .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?,
    })
}

fn add(total: u64, value: usize) -> Result<u64, AbstractSpillAccessConstraintError> {
    total
        .checked_add(cast(value)?)
        .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)
}

fn cast(value: usize) -> Result<u64, AbstractSpillAccessConstraintError> {
    u64::try_from(value).map_err(|_| AbstractSpillAccessConstraintError::WorkOverflow)
}
