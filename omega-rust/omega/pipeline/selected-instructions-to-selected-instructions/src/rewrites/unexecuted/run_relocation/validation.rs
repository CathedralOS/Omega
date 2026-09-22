use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RunRelocationError, RunRelocationReceipt, ValidatedRunRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{CrossingDirection, all_edges, crossed_window};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the touched block, the contiguous run's bounding positions,
/// and the destination index outside it. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    first_index: usize,
    last_index: usize,
    destination_index: usize,
}

/// Map the shared audit's rejection onto this module's public error the
/// same way the producer does: an unschedulable member or crossed
/// position is `UnsupportedInstruction`; every other refusal is
/// `UnsupportedPair`. The unreachable and edge kinds cannot arise on an
/// in-block move, whose crossing is reachable by construction and crosses
/// no edge.
fn reject(rejection: RunRelocationRejection) -> RunRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => RunRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => RunRelocationError::UnsupportedPair,
    }
}

/// Reconstruct the legality of moving the run `first_member`..`last_member`
/// onto `destination` from the source records: locate all three by
/// identity, hand the window they bound to the shared derivation and
/// run-relocation audit — every run member and crossed position
/// schedulable, the roster-carrying run meeting no second accounted actor,
/// every member's hazard directions against each crossed position, and no
/// boundary settlement inside the window — and account the family's
/// measured steps against the budget. Nothing in this audit reads the
/// producer's admission decision, so a producer-side legality error fails
/// here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, RunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RunRelocationError::SourceMismatch)?;
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
        .ok_or(RunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(RunRelocationError::UnsupportedPair)?;
    // The destination sits outside the run in the same block; inside the
    // run it would name a member's own position.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| !(first_index..=last_index).contains(position))
        .ok_or(RunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: an in-block move crosses no edge, so the
    // same-block branch of `crossed_window` derives exactly the positions
    // between the run and the landing, and the direction is inert. The
    // shared audit applies the schedulable, hazard, memory-roster, and
    // settlement checks once — the family's own composition of the shared
    // primitives, not the producer's audit result.
    let edge_limit = all_edges(function).count();
    let crossing = crossed_window(
        function,
        block_index,
        first_index,
        last_index,
        block_index,
        destination_index,
        CrossingDirection::Forward,
        edge_limit,
    )
    .ok_or(RunRelocationError::WorkBudgetExceeded)?;
    let members: Vec<_> = run.iter().collect();
    admit_run_relocation(function, &members, &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: one scan of the plan's body and terminator instructions,
    // every member-against-crossed operand and unit surface, and the
    // function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                crossing
                    .positions
                    .iter()
                    .flat_map(|(_, positions)| positions.iter())
                    .try_fold(total, |total, position| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(&block.instructions[*position]))
                    })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(RunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RunRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        destination_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted run and window from the source without the producer's
/// admission routine, the touched block must place exactly the run's
/// members on the destination's window edge in their original order with
/// the destination instruction adjacent on the vacated side, and
/// rotating the run back must restore the complete source by content —
/// every crossed instruction, every other instruction, register, roster
/// row, call, settlement, and function included.
pub fn validate_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRunRelocation, RunRelocationError> {
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
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .ok_or(RunRelocationError::ReplayMismatch)?;
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    // The run lands on the window edge at the destination's side: leading
    // toward an earlier destination, trailing toward a later one, with the
    // destination instruction always adjacent to the run on the side the
    // run vacated.
    let (run_start, destination_slot) =
        if reconstructed.destination_index < reconstructed.first_index {
            (
                reconstructed.destination_index,
                reconstructed.destination_index + run_len,
            )
        } else {
            (
                reconstructed.destination_index + 1 - run_len,
                reconstructed.destination_index - run_len,
            )
        };
    if proposed_block
        .instructions
        .get(run_start..run_start + run_len)
        != source_block
            .instructions
            .get(reconstructed.first_index..reconstructed.first_index + run_len)
        || proposed_block.instructions.get(destination_slot)
            != source_block
                .instructions
                .get(reconstructed.destination_index)
    {
        return Err(RunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let restored_instructions =
        &mut restored.functions[function_index].blocks[reconstructed.block_index].instructions;
    let run: Vec<_> = restored_instructions
        .drain(run_start..run_start + run_len)
        .collect();
    restored_instructions.splice(reconstructed.first_index..reconstructed.first_index, run);
    if restored != *source.selected_plan() {
        return Err(RunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedRunRelocation {
        receipt: RunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
