use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use selected_instructions::SelectedInstructionPlan;

use crate::{FunctionPressureRematerialization, PressureRematerializationError};

pub(super) fn action_counts(
    functions: &[FunctionPressureRematerialization],
) -> Result<(usize, usize), PressureRematerializationError> {
    let applied = functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    if applied == 0 {
        return Err(PressureRematerializationError::NoAction);
    }
    let rewritten_uses = functions
        .iter()
        .filter_map(|function| function.action.as_ref())
        .try_fold(0usize, |total, action| {
            total.checked_add(action.rewrites.len())
        })
        .ok_or(PressureRematerializationError::WorkOverflow)?;
    Ok((applied, rewritten_uses))
}

pub(crate) fn required_usage(
    selected: &SelectedInstructionPlan,
    applied: usize,
    rewritten_uses: usize,
) -> Result<OptimizationWorkUsage, PressureRematerializationError> {
    let rule_evaluations = u64::try_from(selected.functions.len())
        .map_err(|_| PressureRematerializationError::WorkOverflow)?;
    let validation_steps = selected
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

pub(crate) fn ensure_budget(
    usage: OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
) -> Result<(), PressureRematerializationError> {
    if usage.within(budget) {
        Ok(())
    } else {
        Err(PressureRematerializationError::BudgetExceeded {
            required: usage,
            budget,
        })
    }
}
