use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{CopyRemovalError, CopyRemovalReceipt, ValidatedCopyRemoval, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted copy and its use sites from the source, and the proposal's
/// function must equal the independently computed function — copy removed,
/// destination row dropped, every admitted use rebound to the source,
/// settlements shifted. Reinserting the copy and the roster row and
/// restoring the source settlements must restore the complete source by
/// content — every other instruction, register, call, and function included.
pub fn validate_copy_removal(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    copy: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedCopyRemoval, CopyRemovalError> {
    let admitted = admission::admit(source, function_index, copy, environment, budget)?;
    let mut expected = admitted.function.clone();
    admission::apply(&admitted, &mut expected)?;
    if proposed.functions.get(function_index) != Some(&expected) {
        return Err(CopyRemovalError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    {
        let function = &mut restored.functions[function_index];
        let block = function
            .blocks
            .get_mut(admitted.block_index)
            .ok_or(CopyRemovalError::ReplayMismatch)?;
        // Reinsert the copy first so the recorded use sites keep their
        // source ordinals, then un-rebind them to the destination register.
        block.instructions.insert(
            admitted.copy_index,
            admitted.function.blocks[admitted.block_index].instructions[admitted.copy_index]
                .clone(),
        );
        for site in &admitted.uses {
            super::admission::restore_use(block, site, admitted.output)?;
        }
        if admitted.register_index > function.virtual_registers.len() {
            return Err(CopyRemovalError::ReplayMismatch);
        }
        function.virtual_registers.insert(
            admitted.register_index,
            admitted.function.virtual_registers[admitted.register_index].clone(),
        );
        // The proposed settlements already matched the shifted source, so
        // restoring the source rows restores the source function.
        function.boundary_settlements = admitted.function.boundary_settlements.clone();
    }
    if restored != *source.selected_plan() {
        return Err(CopyRemovalError::ReplayMismatch);
    }
    Ok(ValidatedCopyRemoval {
        receipt: CopyRemovalReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
