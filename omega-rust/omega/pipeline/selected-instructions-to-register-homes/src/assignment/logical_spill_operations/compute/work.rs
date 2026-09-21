use optimization_core::OptimizationWorkUsage;

use crate::{FunctionLogicalSpillOperations, LogicalSpillOperationError};

pub(super) fn usage(
    functions: &[FunctionLogicalSpillOperations],
) -> Result<OptimizationWorkUsage, LogicalSpillOperationError> {
    let function_count =
        u64::try_from(functions.len()).map_err(|_| LogicalSpillOperationError::WorkOverflow)?;
    let planned = functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let planned = u64::try_from(planned).map_err(|_| LogicalSpillOperationError::WorkOverflow)?;
    let rewrites = functions.iter().try_fold(0_u64, |total, function| {
        let count = function
            .action
            .as_ref()
            .map_or(0, |action| action.rewrites.len());
        total
            .checked_add(
                u64::try_from(count).map_err(|_| LogicalSpillOperationError::WorkOverflow)?,
            )
            .ok_or(LogicalSpillOperationError::WorkOverflow)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: planned,
        validation_steps: function_count
            .checked_add(rewrites)
            .ok_or(LogicalSpillOperationError::WorkOverflow)?,
        commits: planned,
        iterations: 1,
    })
}
