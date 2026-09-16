use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{LocalRelocationError, LocalRelocationReceipt, ValidatedLocalRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted window from the source, the touched block must place exactly
/// the member at the destination's index and the destination instruction
/// one slot toward the member's old position, and rotating the member back
/// must restore the complete source by content — every crossed
/// instruction, every other instruction, register, roster row, call,
/// settlement, and function included.
pub fn validate_local_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedLocalRelocation, LocalRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.block_index))
        .ok_or(LocalRelocationError::ReplayMismatch)?;
    if proposed_block.instructions.get(admitted.destination_index)
        != source_block.instructions.get(admitted.member_index)
    {
        return Err(LocalRelocationError::ReplayMismatch);
    }
    // The destination instruction always lands adjacent to the member on
    // the side the member vacated.
    let destination_slot = if admitted.destination_index > admitted.member_index {
        admitted.destination_index - 1
    } else {
        admitted.destination_index + 1
    };
    if proposed_block.instructions.get(destination_slot)
        != source_block.instructions.get(admitted.destination_index)
    {
        return Err(LocalRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let moved = restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .remove(admitted.destination_index);
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .insert(admitted.member_index, moved);
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
