use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{PressureRematerializationError, ValidatedSelectedAnalysis};

pub(super) fn independent_usage(
    selected: &impl ValidatedSelectedAnalysis,
    applied: usize,
    rewritten_uses: usize,
) -> Result<OptimizationWorkUsage, PressureRematerializationError> {
    let rule_evaluations = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| PressureRematerializationError::WorkOverflow)?;
    let validation_steps = selected
        .selected_plan()
        .functions
        .iter()
        .try_fold(0u64, |total, function| {
            let instructions = function.blocks.iter().try_fold(0u64, |count, block| {
                count.checked_add(
                    u64::try_from(block.instructions.len())
                        .ok()?
                        .checked_add(1)?,
                )
            })?;
            total
                .checked_add(u64::try_from(function.virtual_registers.len()).ok()?)?
                .checked_add(instructions)
        })
        .ok_or(PressureRematerializationError::WorkOverflow)?
        .checked_add(
            u64::try_from(rewritten_uses)
                .map_err(|_| PressureRematerializationError::WorkOverflow)?,
        )
        .ok_or(PressureRematerializationError::WorkOverflow)?;
    let applied =
        u64::try_from(applied).map_err(|_| PressureRematerializationError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations,
        candidates: applied,
        validation_steps,
        commits: applied,
        iterations: 1,
    })
}

pub(super) fn validate(
    expected: OptimizationWorkUsage,
    claimed: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), PressureRematerializationError> {
    if claimed != expected {
        return Err(PressureRematerializationError::UsageMismatch);
    }
    if !claimed.within(budget) {
        return Err(PressureRematerializationError::BudgetExceeded {
            required: claimed,
            budget,
        });
    }
    Ok(())
}
