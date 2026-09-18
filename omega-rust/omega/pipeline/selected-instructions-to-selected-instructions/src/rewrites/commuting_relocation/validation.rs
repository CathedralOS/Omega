//! Replay of the in-block commuting relocation: re-admit the window the
//! named member and destination bound from the source, require the
//! proposed program to place exactly the member at the destination's index
//! with the crossed run rotated one slot toward the member's vacated index
//! and the roster equal to the source's own rows permuted into the new
//! execution order, then restore the source by content — rotating the
//! member back and writing the source order's rows back into the window's
//! roster positions must reproduce the complete selected program
//! bit-for-bit.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingRelocationError, CommutingRelocationReceipt, ValidatedCommutingRelocation, admission,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;

/// Independently consume the proposed program: admission re-derives the
/// admitted window from the source, the touched block's window must equal
/// exactly the source window's own instructions rotated — the member at
/// the destination's index, the crossed run one slot toward the member's
/// vacated index — the roster must equal the source's rows permuted into
/// the new execution order, and restoring the window's order and rows must
/// recover the complete source by content: every instruction before and
/// after the window, every other instruction, register, roster row, call,
/// settlement, and function included.
pub fn validate_commuting_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingRelocation, CommutingRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(admitted.block_index)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let first = admitted.member_index.min(admitted.destination_index);
    let last = admitted.member_index.max(admitted.destination_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let member_window_index = admitted.member_index - first;
    // The proposal's window must be the source window's own instructions
    // rotated — the crossed run shifted one slot toward the member's
    // vacated index with the member at the destination's — no other
    // member, order, or content.
    let mut expected: Vec<_> = source_window.to_vec();
    let moved = expected.remove(member_window_index);
    expected.push(moved);
    if admitted.destination_index < admitted.member_index {
        expected.rotate_right(1);
    }
    if proposed_block
        .instructions
        .get(first..=last)
        .map(|window| window.iter().collect::<Vec<_>>())
        != Some(expected.iter().collect())
    {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    // The roster is the family's one content change beyond the rotation:
    // the source's window rows, permuted into the new execution order and
    // nothing else. Comparing against the permutation derived from the
    // source — never from the proposal's own grouping — keeps a stale,
    // reordered, or diluted roster from replaying.
    let window: Vec<SelectedInstructionId> = source_window
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let positions = accesses::window_row_positions(admitted.function, &window);
    let mut new_order = window.clone();
    let moved = new_order.remove(member_window_index);
    new_order.push(moved);
    if admitted.destination_index < admitted.member_index {
        new_order.rotate_right(1);
    }
    let ordered = accesses::rows_in_order(admitted.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    let mut expected_roster = admitted.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let instructions =
        &mut restored.functions[function_index].blocks[admitted.block_index].instructions;
    let moved = instructions.remove(admitted.destination_index);
    instructions.insert(admitted.member_index, moved);
    let restored_function = &mut restored.functions[function_index];
    let source_order = accesses::rows_in_order(admitted.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        restored_function.memory_accesses[*position] = access;
    }
    if restored != *source.selected_plan() {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    Ok(ValidatedCommutingRelocation {
        receipt: CommutingRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
