use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    DeadStoreEliminationError, DeadStoreEliminationReceipt, ValidatedDeadStoreElimination,
    admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the dead
/// pair from the source, and the proposal must equal the source minus exactly
/// the admitted store, its complete roster rows, and the shifted settlement
/// ordinals. Reinserting the store and its rows with the source settlements
/// must restore the complete source by content — every other instruction,
/// register, call, and function included.
pub fn validate_dead_store_elimination(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    store: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDeadStoreElimination, DeadStoreEliminationError> {
    let admitted = admission::admit(source, function_index, store, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(DeadStoreEliminationError::ReplayMismatch)?;
    let source_instructions = &admitted.function.blocks[admitted.block_index].instructions;
    let expected_instructions: Vec<_> = source_instructions
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != admitted.store_index)
        .map(|(_, instruction)| instruction.clone())
        .collect();
    let expected_accesses: Vec<_> = admitted
        .function
        .memory_accesses
        .iter()
        .enumerate()
        .filter(|(index, _)| !admitted.store_accesses.contains(index))
        .map(|(_, access)| access.clone())
        .collect();
    if function
        .blocks
        .get(admitted.block_index)
        .map(|block| &block.instructions)
        != Some(&expected_instructions)
        || function.memory_accesses != expected_accesses
        || function.boundary_settlements
            != admission::shifted_boundary_settlements(
                admitted.function,
                admitted.block,
                admitted.store_index,
            )?
    {
        return Err(DeadStoreEliminationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let restored_function = &mut restored.functions[function_index];
    restored_function.blocks[admitted.block_index]
        .instructions
        .insert(
            admitted.store_index,
            source_instructions[admitted.store_index].clone(),
        );
    // Every removed row reinserts at its own source index, lowest first:
    // the rows below a restored position are already back in place, so each
    // source ordinal is where its row belongs again.
    for &row in &admitted.store_accesses {
        if row > restored_function.memory_accesses.len() {
            return Err(DeadStoreEliminationError::ReplayMismatch);
        }
        restored_function
            .memory_accesses
            .insert(row, admitted.function.memory_accesses[row].clone());
    }
    // The proposed settlements already matched the shifted source, so
    // restoring the source rows restores the source function.
    restored_function.boundary_settlements = admitted.function.boundary_settlements.clone();
    if restored != *source.selected_plan() {
        return Err(DeadStoreEliminationError::ReplayMismatch);
    }
    Ok(ValidatedDeadStoreElimination {
        receipt: DeadStoreEliminationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
