//! Reconstructed producer work accounting.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::RegisterOperandAccess;
use selected_instructions::SelectedInstructionKind;

use crate::rewrites::block_edges::terminator_successors;
use crate::{FunctionLiteralFold, LiteralFoldError, ValidatedSelectedAnalysis};

/// Successor binding slots the nomination occurrence walk visits per block:
/// structural bindings, case payloads, and value bindings on every
/// terminator successor — the same sites the producer's accounting names.
fn successor_sites(function: &selected_instructions::SelectedFunction) -> u64 {
    function
        .blocks
        .iter()
        .flat_map(|block| terminator_successors(&block.terminator).into_iter())
        .map(|successor| {
            successor.structural_bindings.len()
                + successor
                    .structural_case
                    .iter()
                    .map(|case| case.payloads.len())
                    .sum::<usize>()
                + successor.bindings.len()
        })
        .sum::<usize>() as u64
}

pub(super) fn reconstruct_fold_usage(
    selected: &impl ValidatedSelectedAnalysis,
    expected_functions: &[FunctionLiteralFold],
) -> Result<OptimizationWorkUsage, LiteralFoldError> {
    let plan = selected.selected_plan();
    let mut applied = 0_u64;
    let mut validation_steps = plan
        .functions
        .iter()
        .try_fold(0_u64, |total, function| {
            let instructions = function.blocks.iter().try_fold(0_u64, |count, block| {
                count.checked_add(u64::try_from(block.instructions.len() + 1).ok()?)
            })?;
            total
                .checked_add(u64::try_from(function.virtual_registers.len()).ok()?)?
                .checked_add(instructions)
        })
        .ok_or(LiteralFoldError::WorkOverflow)?;
    // The nomination walk the producer charges: once per `MaterializeI64`
    // record in the source, the whole function's instruction sites plus its
    // successor binding slots — whether or not the record's custody
    // admitted it, so the accounting stays a deterministic function of the
    // input.
    for function in &plan.functions {
        let producers = function
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .filter(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::MaterializeI64 { .. }
                )
            })
            .count();
        let per_producer = function
            .blocks
            .iter()
            .try_fold(0_u64, |count, block| {
                count.checked_add(u64::try_from(block.instructions.len() + 1).ok()?)
            })
            .and_then(|sites| sites.checked_add(successor_sites(function)))
            .ok_or(LiteralFoldError::WorkOverflow)?;
        validation_steps = validation_steps
            .checked_add(
                u64::try_from(producers)
                    .map_err(|_| LiteralFoldError::WorkOverflow)?
                    .checked_mul(per_producer)
                    .ok_or(LiteralFoldError::WorkOverflow)?,
            )
            .ok_or(LiteralFoldError::WorkOverflow)?;
    }
    for (function, fold) in plan.functions.iter().zip(expected_functions.iter()) {
        for action in &fold.actions {
            applied = applied
                .checked_add(1)
                .ok_or(LiteralFoldError::WorkOverflow)?;
            // The operand-swapped compare grammar's reader-flow audit ran
            // while the action was derived — re-derive that this action's
            // consumer grammar is the operand-0 fold on `CompareI64` and
            // charge its walk: for each implicit unit the consumer
            // defines, the audit visits a block at most twice — once from
            // the consumer's tail position and once at its head through a
            // back edge — scanning every instruction record plus the
            // terminator and traversing each of the block's successor
            // edges on each visit.
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
            let swapped = consumer.kind == SelectedInstructionKind::CompareI64
                && consumer.operands.first().is_some_and(|operand| {
                    operand.access == RegisterOperandAccess::Use
                        && operand.virtual_register == action.victim
                });
            if !swapped {
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
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: u64::try_from(plan.functions.len())
            .map_err(|_| LiteralFoldError::WorkOverflow)?,
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
