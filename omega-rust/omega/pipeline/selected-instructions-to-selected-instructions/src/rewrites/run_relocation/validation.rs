use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RunRelocationError, RunRelocationReceipt, ValidatedRunRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted run and window from the source, the touched block must place
/// exactly the run's members on the destination's window edge in their
/// original order with the destination instruction adjacent on the vacated
/// side, and rotating the run back must restore the complete source by
/// content — every crossed instruction, every other instruction, register,
/// roster row, call, settlement, and function included.
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
    let admitted = admission::admit(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.block_index))
        .ok_or(RunRelocationError::ReplayMismatch)?;
    let run_len = admitted.last_index - admitted.first_index + 1;
    // The run lands on the window edge at the destination's side: leading
    // toward an earlier destination, trailing toward a later one, with the
    // destination instruction always adjacent to the run on the side the
    // run vacated.
    let (run_start, destination_slot) = if admitted.destination_index < admitted.first_index {
        (
            admitted.destination_index,
            admitted.destination_index + run_len,
        )
    } else {
        (
            admitted.destination_index + 1 - run_len,
            admitted.destination_index - run_len,
        )
    };
    if proposed_block
        .instructions
        .get(run_start..run_start + run_len)
        != source_block
            .instructions
            .get(admitted.first_index..admitted.first_index + run_len)
        || proposed_block.instructions.get(destination_slot)
            != source_block.instructions.get(admitted.destination_index)
    {
        return Err(RunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let restored_instructions =
        &mut restored.functions[function_index].blocks[admitted.block_index].instructions;
    let run: Vec<_> = restored_instructions
        .drain(run_start..run_start + run_len)
        .collect();
    restored_instructions.splice(admitted.first_index..admitted.first_index, run);
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
