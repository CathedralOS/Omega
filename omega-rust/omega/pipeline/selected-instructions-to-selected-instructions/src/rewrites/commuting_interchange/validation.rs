//! Replay of the in-block commuting pair interchange: re-admit the window
//! the named pair bounds from the source, require the proposed program to
//! place exactly the later instruction at the earlier index and the earlier
//! instruction at the later index with the roster equal to the source's
//! own rows permuted into the new execution order, then restore the source
//! by content — swapping the pair back and writing the source order's rows
//! back into the window's roster positions must reproduce the complete
//! selected program bit-for-bit.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingInterchangeError, CommutingInterchangeReceipt, ValidatedCommutingInterchange,
    admission,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;

/// Independently consume the proposed program: admission re-derives the
/// admitted window from the source, the touched block must place exactly
/// the later instruction at the earlier index and the earlier instruction
/// at the later index, the roster must equal the source's rows permuted
/// into the new execution order, and restoring the pair's order and the
/// window's rows must recover the complete source by content — every
/// instruction between them, every other instruction, register, roster
/// row, call, settlement, and function included.
pub fn validate_commuting_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingInterchange, CommutingInterchangeError> {
    let admitted = admission::admit(source, function_index, earlier, later, environment, budget)?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingInterchangeError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(admitted.block_index)
        .ok_or(CommutingInterchangeError::ReplayMismatch)?;
    if proposed_block.instructions.get(admitted.earlier_index)
        != source_block.instructions.get(admitted.later_index)
        || proposed_block.instructions.get(admitted.later_index)
            != source_block.instructions.get(admitted.earlier_index)
    {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    // The roster is the family's one content change beyond the swap: the
    // source's window rows, permuted into the new execution order and
    // nothing else. Comparing against the permutation derived from the
    // source — never from the proposal's own grouping — keeps a stale,
    // reordered, or diluted roster from replaying.
    let window: Vec<SelectedInstructionId> = source_block.instructions
        [admitted.earlier_index..=admitted.later_index]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let positions = accesses::window_row_positions(admitted.function, &window);
    let new_order: Vec<SelectedInstructionId> = std::iter::once(later)
        .chain(window[1..window.len() - 1].iter().copied())
        .chain(std::iter::once(earlier))
        .collect();
    let ordered = accesses::rows_in_order(admitted.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    let mut expected_roster = admitted.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .swap(admitted.earlier_index, admitted.later_index);
    let restored_function = &mut restored.functions[function_index];
    let source_order = accesses::rows_in_order(admitted.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        restored_function.memory_accesses[*position] = access;
    }
    if restored != *source.selected_plan() {
        return Err(CommutingInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedCommutingInterchange {
        receipt: CommutingInterchangeReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
