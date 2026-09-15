use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{AddressFoldError, AddressFoldReceipt, ValidatedAddressFold, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted producer/consumer pair from the source, the proposed consumer
/// instruction must equal the reconstructed folded form, and reinserting
/// the source instruction must restore the complete source by content —
/// every other instruction, register, roster row, call, settlement, and
/// function included.
pub fn validate_address_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    access: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedAddressFold, AddressFoldError> {
    let admitted = admission::admit(source, function_index, access, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(AddressFoldError::ReplayMismatch)?;
    if function
        .blocks
        .get(admitted.block_index)
        .and_then(|block| block.instructions.get(admitted.consumer_index))
        != Some(&admission::rewritten(&admitted))
    {
        return Err(AddressFoldError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.consumer_index] = admitted.function.blocks[admitted.block_index].instructions
        [admitted.consumer_index]
        .clone();
    if restored != *source.selected_plan() {
        return Err(AddressFoldError::ReplayMismatch);
    }
    Ok(ValidatedAddressFold {
        receipt: AddressFoldReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
