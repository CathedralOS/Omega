//! Producer work accounting and budget admission.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{LiteralFoldError, ValidatedSelectedAnalysis};

pub(super) fn fold_usage(
    selected: &impl ValidatedSelectedAnalysis,
    applied: usize,
) -> Result<OptimizationWorkUsage, LiteralFoldError> {
    let functions = u64::try_from(selected.selected_plan().functions.len())
        .map_err(|_| LiteralFoldError::WorkOverflow)?;
    let validation_steps = selected
        .selected_plan()
        .functions
        .iter()
        .try_fold(0_u64, |total, function| {
            let instructions = function.blocks.iter().try_fold(0_u64, |count, block| {
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
        .ok_or(LiteralFoldError::WorkOverflow)?;
    let applied = u64::try_from(applied).map_err(|_| LiteralFoldError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates: applied,
        validation_steps,
        commits: applied,
        iterations: 1,
    })
}

pub(super) fn ensure_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), LiteralFoldError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(LiteralFoldError::BudgetExceeded {
            required: usage,
            budget,
        })
    }
}
