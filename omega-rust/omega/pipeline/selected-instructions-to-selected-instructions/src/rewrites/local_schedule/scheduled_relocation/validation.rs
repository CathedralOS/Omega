use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    ScheduledRelocationError, ScheduledRelocationReceipt, ValidatedScheduledRelocation, admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted run, derived region, and landing index from the source, the
/// proposal must place exactly the run's instructions, contiguous and in
/// order, on the landing index in the destination block, and moving them
/// back must restore the complete source by content — every crossed
/// instruction, every other block and instruction, register, roster row,
/// call, settlement, and edge included.
pub fn validate_scheduled_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedScheduledRelocation, ScheduledRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        members,
        destination,
        environment,
        budget,
    )?;
    let span = admitted.member_last - admitted.member_first + 1;
    let source_run: Vec<&_> = admitted.function.blocks[admitted.block_index].instructions
        [admitted.member_first..=admitted.member_last]
        .iter()
        .collect();
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(ScheduledRelocationError::ReplayMismatch)?;
    let proposed_target = proposed_function
        .blocks
        .get(admitted.target_index)
        .ok_or(ScheduledRelocationError::ReplayMismatch)?;
    let landed = proposed_target.instructions.get(admitted.landing_index..);
    if landed.map(|tail| tail.len() < span).unwrap_or(true)
        || source_run.iter().enumerate().any(|(offset, member)| {
            proposed_target.instructions[admitted.landing_index + offset] != **member
        })
    {
        return Err(ScheduledRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let vacated: Vec<_> = restored.functions[function_index].blocks[admitted.target_index]
        .instructions
        .drain(admitted.landing_index..admitted.landing_index + span)
        .collect();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .splice(admitted.member_first..admitted.member_first, vacated);
    if restored != *source.selected_plan() {
        return Err(ScheduledRelocationError::ReplayMismatch);
    }
    Ok(ValidatedScheduledRelocation {
        receipt: ScheduledRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
