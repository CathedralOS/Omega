use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    StoreMutationMotionError, StoreMutationMotionReceipt, ValidatedStoreMutationMotion, admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// motion's landing position from the source, and the proposal must equal the
/// source with exactly the admitted store moved there — same roster, shifted
/// target-block settlements. Moving the store back with the source
/// settlements must restore the complete source by content — every other
/// instruction, register, call, and function included.
pub fn validate_store_mutation_motion(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedStoreMutationMotion, StoreMutationMotionError> {
    let admitted = admission::admit(source, function_index, store, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(StoreMutationMotionError::ReplayMismatch)?;
    let source_instructions = &admitted.function.blocks[admitted.block_index].instructions;
    let moved_instruction = source_instructions[admitted.store_index].clone();
    if admitted.target_block_index == admitted.block_index {
        let mut expected = source_instructions.clone();
        expected.remove(admitted.store_index);
        expected.insert(admitted.insert_index, moved_instruction.clone());
        if function
            .blocks
            .get(admitted.block_index)
            .map(|block| &block.instructions)
            != Some(&expected)
        {
            return Err(StoreMutationMotionError::ReplayMismatch);
        }
    } else {
        let mut expected_source = source_instructions.clone();
        expected_source.remove(admitted.store_index);
        let mut expected_target = admitted.function.blocks[admitted.target_block_index]
            .instructions
            .clone();
        expected_target.insert(admitted.insert_index, moved_instruction.clone());
        if function
            .blocks
            .get(admitted.block_index)
            .map(|block| &block.instructions)
            != Some(&expected_source)
            || function
                .blocks
                .get(admitted.target_block_index)
                .map(|block| &block.instructions)
                != Some(&expected_target)
        {
            return Err(StoreMutationMotionError::ReplayMismatch);
        }
    }
    // The roster names instructions by identity, so the move retains it
    // unchanged; only boundary settlements remap.
    if function.memory_accesses != admitted.function.memory_accesses
        || function.boundary_settlements
            != admission::shifted_boundary_settlements(
                admitted.function,
                admitted.block,
                admitted.store_index,
                admitted.function.blocks[admitted.target_block_index].id,
                admitted.insert_index,
            )?
    {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    let moved_back = restored_function.blocks[admitted.target_block_index]
        .instructions
        .remove(admitted.insert_index);
    restored_function.blocks[admitted.block_index]
        .instructions
        .insert(admitted.store_index, moved_back);
    restored_function.boundary_settlements = admitted.function.boundary_settlements.clone();
    if restored != *source.selected_plan() {
        return Err(StoreMutationMotionError::ReplayMismatch);
    }
    Ok(ValidatedStoreMutationMotion {
        receipt: StoreMutationMotionReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
