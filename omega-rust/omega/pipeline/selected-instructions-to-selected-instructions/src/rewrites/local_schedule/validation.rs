use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{LocalScheduleError, LocalScheduleReceipt, ValidatedLocalSchedule};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable};

/// The validator's own reconstruction of the interchange the contract
/// permits: the touched block plus the two positions the named pair
/// occupies inside it. It shares no state with the producer's
/// `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    earlier_index: usize,
    later_index: usize,
}

/// Reconstruct the legality of interchanging `earlier` and `later` from
/// the source records: locate both instructions by identity, re-derive
/// the window's independence audit — both members and every interior
/// position schedulable, the roster-carrying member meeting no second
/// accounted actor, every hazard direction of both members against each
/// interior position and against each other, and no boundary settlement
/// inside the window — and account the family's measured steps against
/// the budget. Nothing in this audit reads the producer's admission
/// decision, so a producer-side legality error fails here even when the
/// proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, LocalScheduleError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LocalScheduleError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LocalScheduleError::SourceMismatch)?;
    let (block_index, earlier_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == earlier)
                .map(|earlier_index| (block_index, earlier_index))
        })
        .ok_or(LocalScheduleError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let later_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later)
        .filter(|position| *position > earlier_index)
        .ok_or(LocalScheduleError::UnsupportedPair)?;
    let earlier_instruction = &block.instructions[earlier_index];
    let later_instruction = &block.instructions[later_index];
    let earlier_accounted = schedulable(function, earlier_instruction)
        .ok_or(LocalScheduleError::UnsupportedInstruction)?;
    let later_accounted = schedulable(function, later_instruction)
        .ok_or(LocalScheduleError::UnsupportedInstruction)?;
    if earlier_accounted && later_accounted {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    let member_accounted = earlier_accounted || later_accounted;
    let window = &block.instructions[earlier_index..=later_index];
    for interior in &window[1..window.len() - 1] {
        let interior_accounted =
            schedulable(function, interior).ok_or(LocalScheduleError::UnsupportedInstruction)?;
        if interior_accounted && member_accounted {
            return Err(LocalScheduleError::UnsupportedPair);
        }
        if coupled(earlier_instruction, interior) || coupled(later_instruction, interior) {
            return Err(LocalScheduleError::UnsupportedPair);
        }
    }
    if interior_settlement(function, block.id, earlier_index + 1..=later_index) {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    if coupled(earlier_instruction, later_instruction) {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    // The validator's own audit walks the same surfaces the family
    // publishes: one scan of the plan's body and terminator instructions,
    // every crossed member's operand and unit lists, and the function's
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
        .ok_or(LocalScheduleError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LocalScheduleError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LocalScheduleError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        earlier_index,
        later_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted window from the source without the producer's admission
/// routine, the touched block must place exactly the later instruction
/// at the earlier index and the earlier instruction at the later index,
/// and swapping the pair back must restore the complete source by
/// content — every instruction between them, every other instruction,
/// register, roster row, call, settlement, and function included.
pub fn validate_local_schedule(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedLocalSchedule, LocalScheduleError> {
    let reconstructed = reconstruct(source, function_index, earlier, later, environment, budget)?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.block_index))
        .ok_or(LocalScheduleError::ReplayMismatch)?;
    if proposed_block.instructions.get(reconstructed.earlier_index)
        != source_block.instructions.get(reconstructed.later_index)
        || proposed_block.instructions.get(reconstructed.later_index)
            != source_block.instructions.get(reconstructed.earlier_index)
    {
        return Err(LocalScheduleError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .swap(reconstructed.earlier_index, reconstructed.later_index);
    if restored != *source.selected_plan() {
        return Err(LocalScheduleError::ReplayMismatch);
    }
    Ok(ValidatedLocalSchedule {
        receipt: LocalScheduleReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
