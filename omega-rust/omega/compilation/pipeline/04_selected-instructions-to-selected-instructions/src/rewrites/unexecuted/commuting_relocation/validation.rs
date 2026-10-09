//! Independent validation of the in-block commuting run relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the named run's span
//! inside one block, the destination's position outside the run, the
//! schedulable bar every window position meets, the register and
//! condition-state hazards between each member and every crossed
//! position, the commutation of every roster row that newly trades
//! order, the rowed-trading-pair accounting that keeps this family
//! disjoint from the plain run relocation, and the absence of a boundary
//! settlement inside the window's span — then requires the proposal to
//! place exactly the run's members on the destination's window edge in
//! their original order with the crossed positions shifted one run-width
//! toward the vacated span and the roster equal to the source's own rows
//! permuted into the new execution order, and restores the source by
//! content. Restoring the window's order and rows must reproduce the
//! complete selected program bit-for-bit. A producer admission error
//! therefore fails validation even when the proposal is exactly what
//! that producer emitted.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};

use super::{CommutingRelocationError, CommutingRelocationReceipt, ValidatedCommutingRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;
use crate::rewrites::unexecuted::commuting_accesses as accesses;

/// Independently consume the proposed program: the validator's own
/// record re-derives the admitted run and window from the source, the
/// touched block's window must equal exactly the source window's own
/// instructions rotated — the run on the destination's window edge, the
/// crossed positions one run-width toward the vacated span in their
/// original order — the roster must equal the source's rows permuted
/// into the new execution order, and restoring the window's order and
/// rows must recover the complete source by content: every instruction
/// before and after the window, every other instruction, register,
/// roster row, call, settlement, and function included. The producer's
/// `admission::admit` is never consulted, so a wrong legality decision
/// fails here even when the proposal matches the edit the producer
/// emitted.
pub(crate) fn validate_commuting_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingRelocation, CommutingRelocationError> {
    let reconstructed = super::reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(reconstructed.block_index)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let first = reconstructed
        .first_index
        .min(reconstructed.destination_index);
    let last = reconstructed
        .last_index
        .max(reconstructed.destination_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    // The proposal's window must be the source window's own instructions
    // rotated — the leading group moving to the window's far edge: the
    // crossed prefix when the run moved up, the run itself when it moved
    // down — no other member, order, or content.
    let mut expected: Vec<_> = source_window.to_vec();
    if reconstructed.destination_index < reconstructed.first_index {
        expected.rotate_left(reconstructed.first_index - first);
    } else {
        expected.rotate_left(run_len);
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
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let mut new_order = window.clone();
    if reconstructed.destination_index < reconstructed.first_index {
        new_order.rotate_left(reconstructed.first_index - first);
    } else {
        new_order.rotate_left(run_len);
    }
    let ordered = accesses::rows_in_order(reconstructed.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    let mut expected_roster = reconstructed.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    let restored_function = &mut restored.functions[function_index];
    let source_order = accesses::rows_in_order(reconstructed.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        restored_function.memory_accesses[*position] = access;
    }
    if restored != *source.selected_plan() {
        return Err(CommutingRelocationError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedCommutingRelocation,
        CommutingRelocationReceipt,
        source,
        proposed
    ))
}
