//! Producer-local exact work accounting.

use optimization_core::OptimizationWorkUsage;

use crate::{AbstractSpillAccessConstraintError, FunctionAbstractSpillAccessConstraints};

pub(super) fn usage(
    functions: &[FunctionAbstractSpillAccessConstraints],
) -> Result<OptimizationWorkUsage, AbstractSpillAccessConstraintError> {
    let function_count = count(functions.len())?;
    let placement_count = sum(functions.iter().map(|row| row.placements.len()))?;
    let dependency_count = sum(functions.iter().map(|row| row.dependencies.len()))?;
    let pair_count = sum(functions.iter().map(|row| {
        row.placements
            .iter()
            .enumerate()
            .map(|(index, left)| {
                row.placements[index + 1..]
                    .iter()
                    .filter(|right| right.block == left.block)
                    .count()
            })
            .sum()
    }))?;
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

fn sum(mut values: impl Iterator<Item = usize>) -> Result<u64, AbstractSpillAccessConstraintError> {
    values.try_fold(0_u64, |total, value| {
        total
            .checked_add(count(value)?)
            .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)
    })
}

fn count(value: usize) -> Result<u64, AbstractSpillAccessConstraintError> {
    u64::try_from(value).map_err(|_| AbstractSpillAccessConstraintError::WorkOverflow)
}
