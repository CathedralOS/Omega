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
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{CommutingRelocationError, CommutingRelocationReceipt, ValidatedCommutingRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;
use crate::rewrites::unexecuted::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the admitted run's and destination's coordinates inside
/// their block. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The index whose instruction the run displaces: the window the
    /// relocation crosses is `first_index..=destination_index` in either
    /// order, with the destination always outside the run.
    destination_index: usize,
}

/// Reconstruct the legality of relocating the run `first_member` and
/// `last_member` bound to `destination` from first principles: locate
/// the run and the destination in one block in the named order, require
/// every window position schedulable, refuse any register or
/// condition-state coupling between a member and a crossed position and
/// any roster row that does not commute with a crossed position's row,
/// require at least one rowed trading pair — the accounting case that
/// keeps this family disjoint — and refuse a boundary settlement inside
/// the window's span. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, CommutingRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingRelocationError::SourceMismatch)?;
    let (block_index, first_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == first_member)
                .map(|first_index| (block_index, first_index))
        })
        .ok_or(CommutingRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction relocation the sibling family already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position >= first_index)
        .ok_or(CommutingRelocationError::UnsupportedPair)?;
    // The destination sits outside the run in the same block; inside the
    // run it would name a member's own position, and outside the block it
    // names no in-block window.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| !(first_index..=last_index).contains(position))
        .ok_or(CommutingRelocationError::UnsupportedPair)?;
    let (first, last) = (
        first_index.min(destination_index),
        last_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    let run_len = last_index - first_index + 1;
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the run
    // relocation enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. What changes here is only the accounting
    // rule — a roster-carrying member may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position).ok_or(CommutingRelocationError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every crossed position, so each
    // direction of every register and condition-state hazard applies
    // against each pair, and every row the member carries must commute
    // with every row the crossed position carries. Members of the run keep
    // their relative order and never face this audit against each other,
    // and crossed positions keep their relative order with each other. A
    // window in which no trading pair is rowed on both sides is the run
    // relocation's own accounting case and stays with it.
    let mut rowed_trade = false;
    for (member_index, member) in window.iter().enumerate() {
        if !(first_index - first..first_index - first + run_len).contains(&member_index) {
            continue;
        }
        for (crossed_index, crossed) in window.iter().enumerate() {
            if (first_index - first..first_index - first + run_len).contains(&crossed_index) {
                continue;
            }
            if coupled(member, crossed) {
                return Err(CommutingRelocationError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingRelocationError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingRelocationError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(CommutingRelocationError::UnsupportedPair);
    }
    // The audit walks the plan's body and terminator instructions once;
    // the window audit touches every member-against-crossed operand,
    // unit, and roster-row surface plus the function's three rosters.
    let run_range = first_index - first..first_index - first + run_len;
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window
                .iter()
                .enumerate()
                .try_fold(total, |total, (member_index, member)| {
                    if !run_range.contains(&member_index) {
                        return Some(total);
                    }
                    window
                        .iter()
                        .enumerate()
                        .try_fold(total, |total, (crossed_index, crossed)| {
                            if run_range.contains(&crossed_index) {
                                return Some(total);
                            }
                            total
                                .checked_add(surface(member))?
                                .checked_add(surface(crossed))?
                                .checked_add(
                                    rows[member_index]
                                        .len()
                                        .checked_mul(rows[crossed_index].len())?,
                                )
                        })
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(CommutingRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        destination_index,
    })
}

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
pub fn validate_commuting_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCommutingRelocation, CommutingRelocationError> {
    let reconstructed = reconstruct(
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
