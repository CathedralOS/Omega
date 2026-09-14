use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    StoredLoadForwardingError, StoredLoadForwardingReceipt, ValidatedStoredLoadForwarding,
    admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the pair
/// from the source, the rewritten instruction must equal the reconstructed
/// copy, and the roster must drop exactly the load's read row. Reinserting
/// the load and its row must restore the complete source by content — every
/// other instruction, register, call, settlement, and function included.
pub fn validate_stored_load_forwarding(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    load: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedStoredLoadForwarding, StoredLoadForwardingError> {
    let admitted = admission::admit(source, function_index, load, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(StoredLoadForwardingError::ReplayMismatch)?;
    if function
        .blocks
        .get(admitted.block_index)
        .and_then(|block| block.instructions.get(admitted.load_index))
        != Some(&admission::forwarded(&admitted))
        || function.memory_accesses.len() + 1 != admitted.function.memory_accesses.len()
    {
        return Err(StoredLoadForwardingError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    restored_function.blocks[admitted.block_index].instructions[admitted.load_index] =
        admitted.function.blocks[admitted.block_index].instructions[admitted.load_index].clone();
    if admitted.load_access > restored_function.memory_accesses.len() {
        return Err(StoredLoadForwardingError::ReplayMismatch);
    }
    restored_function.memory_accesses.insert(
        admitted.load_access,
        admitted.function.memory_accesses[admitted.load_access].clone(),
    );
    if restored != *source.selected_plan() {
        return Err(StoredLoadForwardingError::ReplayMismatch);
    }
    Ok(ValidatedStoredLoadForwarding {
        receipt: StoredLoadForwardingReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
