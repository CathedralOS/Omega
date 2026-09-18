use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RunInterchangeError, RunInterchangeReceipt, ValidatedRunInterchange, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted runs and window from the source, the touched block's window
/// must equal exactly the later run, the interior, and the earlier run in
/// that order, and splicing the source window back must restore the
/// complete source by content — every instruction before and after the
/// window, every other instruction, register, roster row, call,
/// settlement, and function included.
pub fn validate_run_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRunInterchange, RunInterchangeError> {
    let admitted = admission::admit(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
    )?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.block_index))
        .ok_or(RunInterchangeError::ReplayMismatch)?;
    let first = admitted.earlier_first_index;
    let last = admitted.later_last_index;
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(RunInterchangeError::ReplayMismatch)?;
    let earlier_len = admitted.earlier_last_index - first + 1;
    let later_len = last - admitted.later_first_index + 1;
    let interior_len = source_window.len() - earlier_len - later_len;
    // The proposal's window must be the source window's own instructions
    // rearranged to later-run ++ interior ++ earlier-run — no other member,
    // order, or content.
    let expected: Vec<_> = source_window[earlier_len + interior_len..]
        .iter()
        .chain(&source_window[earlier_len..earlier_len + interior_len])
        .chain(&source_window[..earlier_len])
        .collect();
    if proposed_block
        .instructions
        .get(first..=last)
        .map(|window| window.iter().collect::<Vec<_>>())
        != Some(expected)
    {
        return Err(RunInterchangeError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    if restored != *source.selected_plan() {
        return Err(RunInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedRunInterchange {
        receipt: RunInterchangeReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
