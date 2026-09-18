use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{BoundaryBooleanError, BoundaryBooleanReceipt, ValidatedBoundaryBoolean, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted pair from the source, the rewritten instruction must equal the
/// reconstructed materialization or equality collapse, and reinserting the
/// source reader must restore the complete source by content — every other
/// instruction, register, roster row, call, settlement, and function
/// included.
pub fn validate_boundary_boolean_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    materialization: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBoundaryBoolean, BoundaryBooleanError> {
    let admitted = admission::admit(source, function_index, materialization, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(BoundaryBooleanError::ReplayMismatch)?;
    if function
        .blocks
        .get(admitted.block_index)
        .and_then(|block| block.instructions.get(admitted.materialization_index))
        != Some(&admission::rewritten(&admitted))
    {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.materialization_index] = admitted.function.blocks[admitted.block_index]
        .instructions[admitted.materialization_index]
        .clone();
    if restored != *source.selected_plan() {
        return Err(BoundaryBooleanError::ReplayMismatch);
    }
    Ok(ValidatedBoundaryBoolean {
        receipt: BoundaryBooleanReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
