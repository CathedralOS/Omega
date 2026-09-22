use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RunInterchangeError, RunInterchangeReceipt, ValidatedRunInterchange};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

/// The validator's own reconstruction of the interchange the contract
/// permits: the touched block plus the four positions the named members
/// bound inside it. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    earlier_first_index: usize,
    earlier_last_index: usize,
    later_first_index: usize,
    later_last_index: usize,
}

/// Reconstruct the legality of trading two disjoint runs from the source
/// records: locate each named member by identity inside one block's body,
/// re-derive the window's independence audit — every run member and
/// interior position schedulable, a roster-carrying run crossing only
/// row-less positions, every hazard direction between each member and the
/// positions outside its own run, and no boundary settlement inside the
/// window's span — and account the family's measured steps against the
/// budget. Nothing in this audit reads the producer's admission decision,
/// so a producer-side legality error fails here even when the proposal
/// matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, RunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RunInterchangeError::SourceMismatch)?;
    let (block_index, earlier_first_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == earlier_first)
                .map(|earlier_first_index| (block_index, earlier_first_index))
        })
        .ok_or(RunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // Each run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the pair
    // interchange's granularity and a repeated or absent id names no run.
    let earlier_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == earlier_last)
        .filter(|position| *position > earlier_first_index)
        .ok_or(RunInterchangeError::UnsupportedPair)?;
    // The later run follows the earlier run disjointly; the positions
    // between them form the interior, which keeps its relative order while
    // the runs trade places.
    let later_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later_first)
        .filter(|position| *position > earlier_last_index)
        .ok_or(RunInterchangeError::UnsupportedPair)?;
    let later_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later_last)
        .filter(|position| *position > later_first_index)
        .ok_or(RunInterchangeError::UnsupportedPair)?;
    let earlier_run = &block.instructions[earlier_first_index..=earlier_last_index];
    let interior = &block.instructions[earlier_last_index + 1..later_first_index];
    let later_run = &block.instructions[later_first_index..=later_last_index];
    // Every member meets the schedulable bar itself: no barrier kind, no
    // call contract, and no unaccounted memory reach. A run's memory
    // accounting is the union of its members' roster rows — roster-carrying
    // members of one run keep their relative order, so a run may carry
    // several rows, but a row-carrying run may only cross row-less
    // positions: a second accounted actor in the other run or the interior
    // would reorder recorded accesses.
    let mut earlier_accounted = false;
    for member in earlier_run {
        earlier_accounted |=
            schedulable(function, member).ok_or(RunInterchangeError::UnsupportedInstruction)?;
    }
    let mut later_accounted = false;
    for member in later_run {
        later_accounted |=
            schedulable(function, member).ok_or(RunInterchangeError::UnsupportedInstruction)?;
    }
    if earlier_accounted && later_accounted {
        return Err(RunInterchangeError::UnsupportedPair);
    }
    let member_accounted = earlier_accounted || later_accounted;
    for crossed in interior {
        // Every interior position meets the same schedulable bar as the
        // members, since every member trades order with it. Its roster
        // rows may stay only while no member of either run carries any —
        // the interior's recorded accesses then keep their relative order
        // while two memory-inert runs trade places around them.
        let interior_accounted =
            schedulable(function, crossed).ok_or(RunInterchangeError::UnsupportedInstruction)?;
        if interior_accounted && member_accounted {
            return Err(RunInterchangeError::UnsupportedPair);
        }
    }
    // Every member trades order with every window position outside its own
    // run — the other run's members and the whole interior — so each
    // direction of every hazard applies against each pair. Members of one
    // run keep their relative order and never face this audit against each
    // other.
    for member in earlier_run {
        for crossed in interior.iter().chain(later_run.iter()) {
            if coupled(member, crossed) {
                return Err(RunInterchangeError::UnsupportedPair);
            }
        }
    }
    for member in later_run {
        for crossed in interior.iter().chain(earlier_run.iter()) {
            if coupled(member, crossed) {
                return Err(RunInterchangeError::UnsupportedPair);
            }
        }
    }
    if interior_settlement(
        function,
        block.id,
        earlier_first_index + 1..=later_last_index,
    ) {
        return Err(RunInterchangeError::UnsupportedPair);
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
            earlier_run.iter().try_fold(total, |total, member| {
                interior
                    .iter()
                    .chain(later_run.iter())
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            later_run.iter().try_fold(total, |total, member| {
                interior
                    .iter()
                    .chain(earlier_run.iter())
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(member))?
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
        .ok_or(RunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RunInterchangeError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        earlier_first_index,
        earlier_last_index,
        later_first_index,
        later_last_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted runs and window from the source without the producer's
/// admission routine, the touched block's window must equal exactly the
/// later run, the interior, and the earlier run in that order, and
/// splicing the source window back must restore the complete source by
/// content — every instruction before and after the window, every other
/// instruction, register, roster row, call, settlement, and function
/// included.
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
    let reconstructed = reconstruct(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
    )?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .ok_or(RunInterchangeError::ReplayMismatch)?;
    let first = reconstructed.earlier_first_index;
    let last = reconstructed.later_last_index;
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(RunInterchangeError::ReplayMismatch)?;
    let earlier_len = reconstructed.earlier_last_index - first + 1;
    let later_len = last - reconstructed.later_first_index + 1;
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
    restored.functions[function_index].blocks[reconstructed.block_index]
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
