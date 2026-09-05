use omega_optimization_core::OptimizationWorkUsage;

use crate::{FunctionStackSlotColoring, StackSlotColoringError};

pub(super) fn usage(
    functions: &[FunctionStackSlotColoring],
) -> Result<OptimizationWorkUsage, StackSlotColoringError> {
    let function_count =
        u64::try_from(functions.len()).map_err(|_| StackSlotColoringError::WorkOverflow)?;
    let assignment_count = functions.iter().try_fold(0_u64, |total, function| {
        total
            .checked_add(
                u64::try_from(function.assignments.len())
                    .map_err(|_| StackSlotColoringError::WorkOverflow)?,
            )
            .ok_or(StackSlotColoringError::WorkOverflow)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: assignment_count,
        validation_steps: function_count
            .checked_add(assignment_count)
            .ok_or(StackSlotColoringError::WorkOverflow)?,
        commits: assignment_count,
        iterations: 1,
    })
}
