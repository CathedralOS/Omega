//! Replay of the in-block commuting member-against-run interchange:
//! re-admit the window the named member and run bound from the source,
//! require the proposed program to place exactly the later side, the
//! interior, and the earlier side in that order inside the window with the
//! roster equal to the source's own rows permuted into the new execution
//! order, then restore the source by content — splicing the source window
//! back and writing the source order's rows back into the window's roster
//! positions must reproduce the complete selected program bit-for-bit.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingMemberRunInterchangeError, CommutingMemberRunInterchangeReceipt,
    ValidatedCommutingMemberRunInterchange, admission,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;

/// Independently consume the proposed program: admission re-derives the
/// admitted member, run, and window from the source, the touched block's
/// window must equal exactly the later side, the interior, and the earlier
/// side in that order — the member and the run exchanged, the interior
/// between them keeping its relative order — the roster must equal the
/// source's rows permuted into the new execution order, and restoring the
/// window's order and rows must recover the complete source by content:
/// every instruction before and after the window, every other instruction,
/// register, roster row, call, settlement, and function included.
pub fn validate_commuting_member_run_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingMemberRunInterchange, CommutingMemberRunInterchangeError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        run_first,
        run_last,
        environment,
        budget,
    )?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingMemberRunInterchangeError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(admitted.block_index)
        .ok_or(CommutingMemberRunInterchangeError::ReplayMismatch)?;
    let first = admitted.member_index.min(admitted.run_first_index);
    let last = admitted.member_index.max(admitted.run_last_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingMemberRunInterchangeError::ReplayMismatch)?;
    let run_len = admitted.run_last_index - admitted.run_first_index + 1;
    let member_earlier = admitted.member_index < admitted.run_first_index;
    let earlier_len = if member_earlier { 1 } else { run_len };
    let later_len = if member_earlier { run_len } else { 1 };
    let interior_len = source_window.len() - earlier_len - later_len;
    // The proposal's window must be the source window's own instructions
    // rearranged to later-side ++ interior ++ earlier-side — no other
    // member, order, or content.
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
        return Err(CommutingMemberRunInterchangeError::ReplayMismatch);
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
        return Err(CommutingMemberRunInterchangeError::ReplayMismatch);
    }
    let mut expected_roster = admitted.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingMemberRunInterchangeError::ReplayMismatch);
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
        return Err(CommutingMemberRunInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedCommutingMemberRunInterchange {
        receipt: CommutingMemberRunInterchangeReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
