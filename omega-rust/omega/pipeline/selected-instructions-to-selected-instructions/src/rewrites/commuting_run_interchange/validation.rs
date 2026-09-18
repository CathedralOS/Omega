//! Replay of the in-block commuting run interchange: re-admit the window
//! the two named runs bound from the source, require the proposed program
//! to place exactly the later run, the interior, and the earlier run in
//! that order inside the window with the roster equal to the source's own
//! rows permuted into the new execution order, then restore the source by
//! content — splicing the source window back and writing the source
//! order's rows back into the window's roster positions must reproduce the
//! complete selected program bit-for-bit.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingRunInterchangeError, CommutingRunInterchangeReceipt, ValidatedCommutingRunInterchange,
    admission,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;

/// Independently consume the proposed program: admission re-derives the
/// admitted runs and window from the source, the touched block's window
/// must equal exactly the later run, the interior, and the earlier run in
/// that order, the roster must equal the source's rows permuted into the
/// new execution order, and restoring the window's order and rows must
/// recover the complete source by content — every instruction before and
/// after the window, every other instruction, register, roster row, call,
/// settlement, and function included.
pub fn validate_commuting_run_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingRunInterchange, CommutingRunInterchangeError> {
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
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingRunInterchangeError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(admitted.block_index)
        .ok_or(CommutingRunInterchangeError::ReplayMismatch)?;
    let first = admitted.earlier_first_index;
    let last = admitted.later_last_index;
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingRunInterchangeError::ReplayMismatch)?;
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
        return Err(CommutingRunInterchangeError::ReplayMismatch);
    }
    // The roster is the family's one content change beyond the reorder:
    // the source's window rows, permuted into the new execution order and
    // nothing else. Comparing against the permutation derived from the
    // source — never from the proposal's own grouping — keeps a stale,
    // reordered, or diluted roster from replaying.
    let window: Vec<SelectedInstructionId> = source_window
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let positions = accesses::window_row_positions(admitted.function, &window);
    let new_order: Vec<SelectedInstructionId> = window[earlier_len + interior_len..]
        .iter()
        .chain(&window[earlier_len..earlier_len + interior_len])
        .chain(&window[..earlier_len])
        .copied()
        .collect();
    let ordered = accesses::rows_in_order(admitted.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(CommutingRunInterchangeError::ReplayMismatch);
    }
    let mut expected_roster = admitted.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingRunInterchangeError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    let restored_function = &mut restored.functions[function_index];
    let source_order = accesses::rows_in_order(admitted.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        restored_function.memory_accesses[*position] = access;
    }
    if restored != *source.selected_plan() {
        return Err(CommutingRunInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedCommutingRunInterchange {
        receipt: CommutingRunInterchangeReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
