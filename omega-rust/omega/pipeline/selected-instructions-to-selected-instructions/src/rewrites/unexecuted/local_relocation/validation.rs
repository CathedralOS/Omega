use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{LocalRelocationError, LocalRelocationReceipt, ValidatedLocalRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{CrossingDirection, crossed_window};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation};

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

/// Map the shared audit's rejection onto this module's public error the
/// same way the producer does.
fn reject(rejection: RunRelocationRejection) -> LocalRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => LocalRelocationError::UnsupportedInstruction,
        _ => LocalRelocationError::UnsupportedPair,
    }
}

/// Reconstruct the legality of moving `member` onto `destination` from the
/// source records: locate both instructions by identity, re-derive the
/// window's independence audit — every crossed position schedulable, the
/// roster-carrying member meeting no second accounted actor, every hazard
/// direction against each crossed position, and no boundary settlement
/// inside the window — and account the family's measured steps against the
/// budget. Nothing in this audit reads the producer's admission decision,
/// so a producer-side legality error fails here even when the proposal
/// matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, LocalRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LocalRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LocalRelocationError::SourceMismatch)?;
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
        .ok_or(LocalRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The window is the span between the member and the destination in
    // this block; the destination at the member's own position names no
    // move.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| *position != member_index)
        .ok_or(LocalRelocationError::UnsupportedPair)?;
    let member_instruction = &block.instructions[member_index];
    // The single member is a one-instruction run: the shared derivation
    // returns the positions between it and the landing index, the
    // destination included, and the shared audit proves that window
    // independent once. An in-block move crosses no edge, so the path
    // bound is inert.
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        block_index,
        destination_index,
        CrossingDirection::Forward,
        0,
    )
    .ok_or(LocalRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: one scan of the plan's body and terminator instructions,
    // every crossed member's operand and unit lists, and the function's
    // three rosters.
    let (first, last) = (
        member_index.min(destination_index),
        member_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window.iter().try_fold(total, |total, instruction| {
                total
                    .checked_add(instruction.operands.len())?
                    .checked_add(instruction.implicit_uses.len())?
                    .checked_add(instruction.implicit_defs.len())?
                    .checked_add(instruction.clobbers.len())
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(LocalRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LocalRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LocalRelocationError::WorkBudgetExceeded);
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
/// routine, the touched block must place exactly the member at the
/// destination's index and the destination instruction one slot toward
/// the member's old position, and rotating the member back must restore
/// the complete source by content — every crossed instruction, every
/// other instruction, register, roster row, call, settlement, and
/// function included.
pub fn validate_local_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedLocalRelocation, LocalRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .ok_or(LocalRelocationError::ReplayMismatch)?;
    if proposed_block
        .instructions
        .get(reconstructed.destination_index)
        != source_block.instructions.get(reconstructed.member_index)
    {
        return Err(LocalRelocationError::ReplayMismatch);
    }
    // The destination instruction always lands adjacent to the member on
    // the side the member vacated.
    let destination_slot = if reconstructed.destination_index > reconstructed.member_index {
        reconstructed.destination_index - 1
    } else {
        reconstructed.destination_index + 1
    };
    if proposed_block.instructions.get(destination_slot)
        != source_block
            .instructions
            .get(reconstructed.destination_index)
    {
        return Err(LocalRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let moved = restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .remove(reconstructed.destination_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, moved);
    if restored != *source.selected_plan() {
        return Err(LocalRelocationError::ReplayMismatch);
    }
    Ok(ValidatedLocalRelocation {
        receipt: LocalRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
