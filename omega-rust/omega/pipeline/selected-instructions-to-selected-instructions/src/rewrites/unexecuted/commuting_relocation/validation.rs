//! Replay of the in-block commuting relocation: the validator re-admits
//! the window the named member and destination bound from the source on
//! its own audit, requires the proposed program to place exactly the
//! member at the destination's index with the crossed run rotated one
//! slot toward the member's vacated index and the roster equal to the
//! source's own rows permuted into the new execution order, then restores
//! the source by content — rotating the member back and writing the
//! source order's rows back into the window's roster positions must
//! reproduce the complete selected program bit-for-bit.
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
/// permits: the touched block plus the member's and destination's
/// positions inside it. It shares no state with the producer's
/// `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    destination_index: usize,
}

/// Reconstruct the legality of moving `member` onto `destination` from the
/// source records: locate both instructions by identity, re-derive the
/// window's independence audit — every position schedulable, every hazard
/// direction between the member and each crossed position, every roster
/// row the member carries commuting with every row a crossed position
/// carries, at least one pair rowed on both sides, and no boundary
/// settlement inside the window — and account the family's measured steps
/// against the budget. Nothing in this audit reads the producer's
/// admission decision, so a producer-side legality error fails here even
/// when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
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
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(CommutingRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The window is the span between the member and the destination in
    // this block; the destination at the member's own position names no
    // move.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| *position != member_index)
        .ok_or(CommutingRelocationError::UnsupportedPair)?;
    let (first, last) = (
        member_index.min(destination_index),
        member_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    let member_window_index = member_index - first;
    let member_instruction = &window[member_window_index];
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the local
    // relocation enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. The accounting rule here is the commuting
    // one — the row-carrying member may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position).ok_or(CommutingRelocationError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // The member trades order with every crossed position — the
    // destination and the whole interior — so each direction of every
    // register and condition-state hazard applies against each, and every
    // row the member carries must commute with every row the crossed
    // position carries. Crossed positions keep their relative order with
    // each other and are never audited against one another. A window in
    // which no trading pair is rowed on both sides is the local
    // relocation's own accounting case and stays with it.
    let mut rowed_trade = false;
    for (crossed_index, crossed) in window.iter().enumerate() {
        if crossed_index == member_window_index {
            continue;
        }
        if coupled(member_instruction, crossed) {
            return Err(CommutingRelocationError::UnsupportedPair);
        }
        rowed_trade |= !rows[member_window_index].is_empty() && !rows[crossed_index].is_empty();
        for member_row in &rows[member_window_index] {
            for crossed_row in &rows[crossed_index] {
                if !accesses::commutes(member_row, crossed_row, structural_places) {
                    return Err(CommutingRelocationError::UnsupportedPair);
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
    // The validator's own audit walks the same surfaces the family
    // publishes: one scan of the plan's body and terminator instructions,
    // the member's surface against each crossed position's operand, unit,
    // and roster-row surface, and the function's three rosters.
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
                .try_fold(total, |total, (crossed_index, crossed)| {
                    if crossed_index == member_window_index {
                        return Some(total);
                    }
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))?
                        .checked_add(
                            rows[member_window_index]
                                .len()
                                .checked_mul(rows[crossed_index].len())?,
                        )
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
        member_index,
        destination_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted window from the source without the producer's admission
/// routine, the touched block's window must equal exactly the source
/// window's own instructions rotated — the member at the destination's
/// index, the crossed run one slot toward the member's vacated index —
/// the roster must equal the source's rows permuted into the new
/// execution order, and restoring the window's order and rows must
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
    let reconstructed = reconstruct(
        source,
        function_index,
        member,
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
        .member_index
        .min(reconstructed.destination_index);
    let last = reconstructed
        .member_index
        .max(reconstructed.destination_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingRelocationError::ReplayMismatch)?;
    let member_window_index = reconstructed.member_index - first;
    // The proposal's window must be the source window's own instructions
    // rotated — the crossed run shifted one slot toward the member's
    // vacated index with the member at the destination's — no other
    // member, order, or content.
    let mut expected: Vec<_> = source_window.to_vec();
    let moved = expected.remove(member_window_index);
    expected.push(moved);
    if reconstructed.destination_index < reconstructed.member_index {
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
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let mut new_order = window.clone();
    let moved = new_order.remove(member_window_index);
    new_order.push(moved);
    if reconstructed.destination_index < reconstructed.member_index {
        new_order.rotate_right(1);
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
    let instructions =
        &mut restored.functions[function_index].blocks[reconstructed.block_index].instructions;
    let moved = instructions.remove(reconstructed.destination_index);
    instructions.insert(reconstructed.member_index, moved);
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
