use omega_optimization_core::OptimizationWorkUsage;

use crate::{FunctionStackSlotColoring, StackSlotColoringError};

pub(super) fn usage(
    functions: &[FunctionStackSlotColoring],
) -> Result<OptimizationWorkUsage, StackSlotColoringError> {
    let function_count =
        u64::try_from(functions.len()).map_err(|_| StackSlotColoringError::WorkOverflow)?;
    let assignments = assignment_count(functions)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: assignments,
        validation_steps: function_count
            .checked_add(assignments)
            .ok_or(StackSlotColoringError::WorkOverflow)?,
        commits: assignments,
        iterations: 1,
    })
}

fn assignment_count(
    functions: &[FunctionStackSlotColoring],
) -> Result<u64, StackSlotColoringError> {
    functions.iter().try_fold(0_u64, |total, function| {
        total
            .checked_add(
                u64::try_from(function.assignments.len())
                    .map_err(|_| StackSlotColoringError::WorkOverflow)?,
            )
            .ok_or(StackSlotColoringError::WorkOverflow)
    })
}
