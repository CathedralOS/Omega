//! Replay of the in-block commuting member-against-run interchange:
//! re-derive the window the named member and run bound from the source,
//! require the proposed program to place exactly the later side, the
//! interior, and the earlier side in that order inside the window with the
//! roster equal to the source's own rows permuted into the new execution
//! order, then restore the source by content — splicing the source window
//! back and writing the source order's rows back into the window's roster
//! positions must reproduce the complete selected program bit-for-bit.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    CommutingMemberRunInterchangeError, CommutingMemberRunInterchangeReceipt,
    ValidatedCommutingMemberRunInterchange,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

/// The validator's own reconstruction of the interchange the contract
/// permits: the touched block plus the three positions the named member
/// and run members bound inside it. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    run_first_index: usize,
    run_last_index: usize,
}

/// Reconstruct the legality of trading a member against a run under
/// proven memory commutation from the source records: locate each named
/// member by identity inside one block's body, then re-derive the
/// window's independence audit — every window position schedulable,
/// every register and condition-state hazard direction between each
/// member and the positions outside its own side, every roster row that
/// newly trades order commuting with every row of the position it
/// crosses, at least one trading pair rowed on both sides — the
/// accounting case that keeps this family disjoint from the plain
/// member-against-run interchange — and no boundary settlement inside
/// the window's span — and account the family's measured steps against
/// the budget. Nothing in this audit reads the producer's admission
/// decision, so a producer-side legality error fails here even when the
/// proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, CommutingMemberRunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingMemberRunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingMemberRunInterchangeError::SourceMismatch)?;
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
        .ok_or(CommutingMemberRunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the commuting
    // pair's granularity and a repeated or absent id names no run.
    let run_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_first)
        .ok_or(CommutingMemberRunInterchangeError::UnsupportedPair)?;
    let run_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_last)
        .filter(|position| *position > run_first_index)
        .ok_or(CommutingMemberRunInterchangeError::UnsupportedPair)?;
    // The member sits strictly on one side of the run's span: inside the
    // span it would be part of the side it trades against, and a member
    // equal to either named bound names no disjoint side.
    if (run_first_index..=run_last_index).contains(&member_index) {
        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
    }
    let (window_first, window_last) = (
        member_index.min(run_first_index),
        member_index.max(run_last_index),
    );
    let window = &block.instructions[window_first..=window_last];
    // The window holds earlier-side ++ interior ++ later-side, where the
    // earlier side is the member when it precedes the run and the run when
    // it follows; the positions between the sides form the interior,
    // possibly empty, which keeps its relative order while the two sides
    // trade places.
    let run_len = run_last_index - run_first_index + 1;
    let member_earlier = member_index < run_first_index;
    let earlier_len = if member_earlier { 1 } else { run_len };
    let later_len = if member_earlier { run_len } else { 1 };
    let interior_len = window.len() - earlier_len - later_len;
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the
    // member-against-run interchange enforces: no barrier kind, no call
    // contract, and no unaccounted memory reach. What changes here is only
    // the accounting rule — a roster-carrying member may trade order with
    // a crossed position whose own rows commute with it.
    for position in window {
        schedulable(function, position)
            .ok_or(CommutingMemberRunInterchangeError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member of each side trades order with every window position
    // outside its own side — the earlier side with the interior and the
    // later side, the later side with the earlier side and the interior —
    // so each direction of every register and condition-state hazard
    // applies against each pair, and every row the member carries must
    // commute with every row the crossed position carries. The run's
    // members keep their relative order and never face this audit against
    // each other, and interior positions keep their relative order with
    // each other. A window in which no trading pair is rowed on both
    // sides is the member-against-run interchange's own accounting case
    // and stays with it.
    let mut rowed_trade = false;
    for (member_window_index, member_instruction) in window.iter().enumerate() {
        let crossed_range = if member_window_index < earlier_len {
            // An earlier-side member crosses the interior and the later
            // side.
            earlier_len..window.len()
        } else if member_window_index < earlier_len + interior_len {
            // An interior position crosses nothing alone: its trades are
            // already audited as the crossed side of both sides' members.
            continue;
        } else {
            // A later-side member crosses the earlier side and the
            // interior.
            0..earlier_len + interior_len
        };
        for crossed_index in crossed_range {
            let crossed = &window[crossed_index];
            if coupled(member_instruction, crossed) {
                return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_window_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_window_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, window_first + 1..=window_last) {
        return Err(CommutingMemberRunInterchangeError::UnsupportedPair);
    }
    // The validator's own audit walks the same surfaces the family
    // publishes: one scan of the plan's body instructions, every
    // member-against-crossed operand, unit, and roster-row surface, and
    // the function's three rosters.
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
                .try_fold(total, |total, (member_window_index, member)| {
                    let mut crossed_range = if member_window_index < earlier_len {
                        earlier_len..window.len()
                    } else if member_window_index < earlier_len + interior_len {
                        return Some(total);
                    } else {
                        0..earlier_len + interior_len
                    };
                    crossed_range.try_fold(total, |total, crossed_index| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(&window[crossed_index]))?
                            .checked_add(
                                rows[member_window_index]
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
        .ok_or(CommutingMemberRunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingMemberRunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingMemberRunInterchangeError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        member_index,
        run_first_index,
        run_last_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted member, run, and window from the source without the
/// producer's admission routine, the touched block's window must equal
/// exactly the later side, the interior, and the earlier side in that
/// order — the member and the run exchanged, the interior between them
/// keeping its relative order — the roster must equal the source's rows
/// permuted into the new execution order, and restoring the window's
/// order and rows must recover the complete source by content: every
/// instruction before and after the window, every other instruction,
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
    let reconstructed = reconstruct(
        source,
        function_index,
        member,
        run_first,
        run_last,
        environment,
        budget,
    )?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(CommutingMemberRunInterchangeError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(reconstructed.block_index)
        .ok_or(CommutingMemberRunInterchangeError::ReplayMismatch)?;
    let first = reconstructed
        .member_index
        .min(reconstructed.run_first_index);
    let last = reconstructed.member_index.max(reconstructed.run_last_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(CommutingMemberRunInterchangeError::ReplayMismatch)?;
    let run_len = reconstructed.run_last_index - reconstructed.run_first_index + 1;
    let member_earlier = reconstructed.member_index < reconstructed.run_first_index;
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
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let new_order: Vec<SelectedInstructionId> = window[earlier_len + interior_len..]
        .iter()
        .chain(&window[earlier_len..earlier_len + interior_len])
        .chain(&window[..earlier_len])
        .copied()
        .collect();
    let ordered = accesses::rows_in_order(reconstructed.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(CommutingMemberRunInterchangeError::ReplayMismatch);
    }
    let mut expected_roster = reconstructed.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(CommutingMemberRunInterchangeError::ReplayMismatch);
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
