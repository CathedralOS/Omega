use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{LocalScheduleError, LocalScheduleReceipt, ValidatedLocalSchedule, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted window from the source, the touched block must place exactly the
/// later instruction at the earlier index and the earlier instruction at the
/// later index, and swapping the pair back must restore the complete source
/// by content — every instruction between them, every other instruction,
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
    let admitted = admission::admit(source, function_index, earlier, later, environment, budget)?;
    let source_block = &admitted.function.blocks[admitted.block_index];
    let proposed_block = proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.block_index))
        .ok_or(LocalScheduleError::ReplayMismatch)?;
    if proposed_block.instructions.get(admitted.earlier_index)
        != source_block.instructions.get(admitted.later_index)
        || proposed_block.instructions.get(admitted.later_index)
            != source_block.instructions.get(admitted.earlier_index)
    {
        return Err(LocalScheduleError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .swap(admitted.earlier_index, admitted.later_index);
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
