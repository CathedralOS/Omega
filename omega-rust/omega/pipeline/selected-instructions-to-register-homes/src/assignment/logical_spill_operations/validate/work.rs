use optimization_core::OptimizationWorkUsage;

use crate::{FunctionLogicalSpillOperations, LogicalSpillOperationError};

pub(super) fn usage(
    functions: &[FunctionLogicalSpillOperations],
) -> Result<OptimizationWorkUsage, LogicalSpillOperationError> {
    let rules =
        u64::try_from(functions.len()).map_err(|_| LogicalSpillOperationError::WorkOverflow)?;
    let actions = functions.iter().try_fold(0_u64, |count, function| {
        count
            .checked_add(u64::from(function.action.is_some()))
            .ok_or(LogicalSpillOperationError::WorkOverflow)
    })?;
    let rewrites = functions.iter().try_fold(0_u64, |count, function| {
        let next = function
            .action
            .as_ref()
            .map_or(0, |action| action.rewrites.len());
        count
            .checked_add(u64::try_from(next).map_err(|_| LogicalSpillOperationError::WorkOverflow)?)
            .ok_or(LogicalSpillOperationError::WorkOverflow)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: rules,
        candidates: actions,
        validation_steps: rules
            .checked_add(rewrites)
            .ok_or(LogicalSpillOperationError::WorkOverflow)?,
        commits: actions,
        iterations: 1,
    })
}
