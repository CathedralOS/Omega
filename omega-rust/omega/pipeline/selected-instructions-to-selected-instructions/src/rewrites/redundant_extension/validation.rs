use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    RedundantExtensionError, RedundantExtensionReceipt, ValidatedRedundantExtension, admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the pair
/// from the source, the rewritten instruction must equal the reconstructed
/// copy, and reinserting the extension must restore the complete source by
/// content — every other instruction, register, roster row, call, settlement,
/// and function included.
pub fn validate_redundant_extension_removal(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    extension: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRedundantExtension, RedundantExtensionError> {
    let admitted = admission::admit(source, function_index, extension, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(RedundantExtensionError::ReplayMismatch)?;
    if function
        .blocks
        .get(admitted.block_index)
        .and_then(|block| block.instructions.get(admitted.extension_index))
        != Some(&admission::rewritten(&admitted))
    {
        return Err(RedundantExtensionError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.extension_index] = admitted.function.blocks[admitted.block_index].instructions
        [admitted.extension_index]
        .clone();
    if restored != *source.selected_plan() {
        return Err(RedundantExtensionError::ReplayMismatch);
    }
    Ok(ValidatedRedundantExtension {
        receipt: RedundantExtensionReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
