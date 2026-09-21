//! Producer work accounting and budget admission.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::RegisterOperandAccess;

use crate::rewrites::block_edges::terminator_successors;
use crate::{
    FunctionLiteralFold, LiteralFoldError, PairUnitDefRelation, ValidatedSelectedAnalysis,
};

use super::constraints::AdmittedPairs;

pub(super) fn fold_usage(
    selected: &impl ValidatedSelectedAnalysis,
    functions: &[FunctionLiteralFold],
    rows: &AdmittedPairs<'_>,
) -> Result<OptimizationWorkUsage, LiteralFoldError> {
    let plan = selected.selected_plan();
    let function_count =
        u64::try_from(plan.functions.len()).map_err(|_| LiteralFoldError::WorkOverflow)?;
    let mut validation_steps = plan
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
    let mut applied = 0_u64;
    for (function, fold) in plan.functions.iter().zip(functions.iter()) {
        let Some(action) = &fold.action else {
            continue;
        };
        applied = applied
            .checked_add(1)
            .ok_or(LiteralFoldError::WorkOverflow)?;
        // The operand-swapped compare grammar's reader-flow audit ran
        // while this action was derived — re-derive that the enabled
        // pair bound to the action's consumer grammar declares
        // `OperandSwapped` and charge its walk: for each
        // implicit unit the consumer defines, the audit visits a block
        // at most twice — once from the consumer's tail position and
        // once at its head through a back edge — scanning every
        // instruction record plus the terminator and traversing each of
        // the block's successor edges on each visit.
        let Some(consumer) = function
            .blocks
            .iter()
            .find(|block| block.id == action.block)
            .and_then(|block| {
                block
                    .instructions
                    .iter()
                    .find(|instruction| instruction.id == action.consumer_instruction)
            })
        else {
            continue;
        };
        let Some(victim_position) = consumer
            .operands
            .iter()
            .position(|operand| {
                operand.access == RegisterOperandAccess::Use
                    && operand.virtual_register == action.victim
            })
            .and_then(|position| u16::try_from(position).ok())
        else {
            continue;
        };
        let audited = rows
            .for_position(consumer.kind, victim_position)
            .is_some_and(|pair| {
                pair.rule.machine_effects().unit_defs == PairUnitDefRelation::OperandSwapped
            });
        if !audited {
            continue;
        }
        let scan = function
            .blocks
            .iter()
            .try_fold(0_u64, |count, block| {
                count.checked_add(u64::try_from(block.instructions.len() + 1).ok()?)
            })
            .ok_or(LiteralFoldError::WorkOverflow)?;
        let edges = function
            .blocks
            .iter()
            .try_fold(0_u64, |count, block| {
                count.checked_add(
                    u64::try_from(terminator_successors(&block.terminator).len()).ok()?,
                )
            })
            .ok_or(LiteralFoldError::WorkOverflow)?;
        validation_steps = validation_steps
            .checked_add(
                u64::try_from(consumer.implicit_defs.len())
                    .map_err(|_| LiteralFoldError::WorkOverflow)?
                    .checked_mul(
                        scan.checked_add(edges)
                            .and_then(|per_visit| per_visit.checked_mul(2))
                            .ok_or(LiteralFoldError::WorkOverflow)?,
                    )
                    .ok_or(LiteralFoldError::WorkOverflow)?,
            )
            .ok_or(LiteralFoldError::WorkOverflow)?;
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
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
