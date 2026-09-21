use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    MemberRunInterchangeError, MemberRunInterchangeReceipt, ValidatedMemberRunInterchange,
};
use crate::ValidatedSelectedAnalysis;
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

/// Reconstruct the legality of trading a member against a run from the
/// source records: locate each named member by identity inside one
/// block's body, re-derive the window's independence audit — every side
/// member and interior position schedulable, a roster-carrying side
/// crossing only row-less positions, every hazard direction between each
/// member and the positions outside its own side, and no boundary
/// settlement inside the window's span — and account the family's
/// measured steps against the budget. Nothing in this audit reads the
/// producer's admission decision, so a producer-side legality error fails
/// here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, MemberRunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(MemberRunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(MemberRunInterchangeError::SourceMismatch)?;
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
        .ok_or(MemberRunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the pair
    // interchange's granularity and a repeated or absent id names no run.
    let run_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_first)
        .ok_or(MemberRunInterchangeError::UnsupportedPair)?;
    let run_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == run_last)
        .filter(|position| *position > run_first_index)
        .ok_or(MemberRunInterchangeError::UnsupportedPair)?;
    // The member sits strictly on one side of the run's span: inside the
    // span it would be part of the side it trades against, and a member
    // equal to either named bound names no disjoint side.
    if (run_first_index..=run_last_index).contains(&member_index) {
        return Err(MemberRunInterchangeError::UnsupportedPair);
    }
    let member_instruction = &block.instructions[member_index];
    let run = &block.instructions[run_first_index..=run_last_index];
    // The positions between the member and the run form the interior,
    // possibly empty, which keeps its relative order while the two sides
    // trade places.
    let interior = if member_index < run_first_index {
        &block.instructions[member_index + 1..run_first_index]
    } else {
        &block.instructions[run_last_index + 1..member_index]
    };
    // Every side member meets the schedulable bar itself: no barrier kind,
    // no call contract, and no unaccounted memory reach. A side's memory
    // accounting is the union of its members' roster rows — roster-carrying
    // members of the run keep their relative order, so the run may carry
    // several rows itself, but a row-carrying side may only cross row-less
    // positions: a second accounted actor on the other side or in the
    // interior would reorder recorded accesses.
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(MemberRunInterchangeError::UnsupportedInstruction)?;
    let mut run_accounted = false;
    for run_member in run {
        run_accounted |= schedulable(function, run_member)
            .ok_or(MemberRunInterchangeError::UnsupportedInstruction)?;
    }
    if member_accounted && run_accounted {
        return Err(MemberRunInterchangeError::UnsupportedPair);
    }
    let side_accounted = member_accounted || run_accounted;
    for crossed in interior {
        // Every interior position meets the same schedulable bar as the
        // side members, since both sides trade order with it. Its roster
        // rows may stay only while neither side carries any — the
        // interior's recorded accesses then keep their relative order
        // while two memory-inert sides trade places around them.
        let interior_accounted = schedulable(function, crossed)
            .ok_or(MemberRunInterchangeError::UnsupportedInstruction)?;
        if interior_accounted && side_accounted {
            return Err(MemberRunInterchangeError::UnsupportedPair);
        }
    }
    // Every side member trades order with every window position outside
    // its own side — the member with the whole run and the interior, each
    // run member with the member and the whole interior — so each
    // direction of every hazard applies against each pair. The run's
    // members keep their relative order and never face this audit against
    // each other.
    for crossed in interior.iter().chain(run.iter()) {
        if coupled(member_instruction, crossed) {
            return Err(MemberRunInterchangeError::UnsupportedPair);
        }
    }
    for run_member in run {
        for crossed in interior.iter().chain(std::iter::once(member_instruction)) {
            if coupled(run_member, crossed) {
                return Err(MemberRunInterchangeError::UnsupportedPair);
            }
        }
    }
    let (window_first, window_last) = (
        member_index.min(run_first_index),
        member_index.max(run_last_index),
    );
    if interior_settlement(function, block.id, window_first + 1..=window_last) {
        return Err(MemberRunInterchangeError::UnsupportedPair);
    }
    // The validator's own audit walks the same surfaces the family
    // publishes: one scan of the plan's body instructions, every
    // member-against-crossed operand and unit surface, and the function's
    // three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            interior
                .iter()
                .chain(run.iter())
                .try_fold(total, |total, crossed| {
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))
                })
        })
        .and_then(|total| {
            run.iter().try_fold(total, |total, run_member| {
                interior
                    .iter()
                    .chain(std::iter::once(member_instruction))
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(run_member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(MemberRunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| MemberRunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(MemberRunInterchangeError::WorkBudgetExceeded);
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
/// keeping its relative order — and splicing the source window back must
/// restore the complete source by content: every instruction before and
/// after the window, every other instruction, register, roster row, call,
/// settlement, and function included.
pub fn validate_member_run_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedMemberRunInterchange, MemberRunInterchangeError> {
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
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .ok_or(MemberRunInterchangeError::ReplayMismatch)?;
    let first = reconstructed
        .member_index
        .min(reconstructed.run_first_index);
    let last = reconstructed.member_index.max(reconstructed.run_last_index);
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(MemberRunInterchangeError::ReplayMismatch)?;
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
        return Err(MemberRunInterchangeError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    if restored != *source.selected_plan() {
        return Err(MemberRunInterchangeError::ReplayMismatch);
    }
    Ok(ValidatedMemberRunInterchange {
        receipt: MemberRunInterchangeReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
