use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{DeadCompareError, DeadCompareReceipt, ValidatedDeadCompare, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted compare and re-audits every published unit's forward paths from
/// the source, the proposal's function must equal the independently
/// computed removal — compare dropped, settlements shifted — and
/// reinserting the compare with the source settlements must restore the
/// complete source by content: every other instruction, register, call,
/// and function included.
pub fn validate_dead_compare(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDeadCompare, DeadCompareError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let mut expected = admitted.function.clone();
    admission::apply(&admitted, &mut expected)?;
    if proposed.functions.get(function_index) != Some(&expected) {
        return Err(DeadCompareError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    {
        let function = &mut restored.functions[function_index];
        let block = function
            .blocks
            .get_mut(admitted.block_index)
            .ok_or(DeadCompareError::ReplayMismatch)?;
        // Reinsertion is valid when the removed ordinal is still inside the
        // body — a compare that ended its block reinserts at its tail.
        if block.instructions.len() < admitted.compare_index {
            return Err(DeadCompareError::ReplayMismatch);
        }
        block.instructions.insert(
            admitted.compare_index,
            admitted.function.blocks[admitted.block_index].instructions[admitted.compare_index]
                .clone(),
        );
        // The proposed settlements already matched the shifted source, so
        // restoring the source rows restores the source function.
        function.boundary_settlements = admitted.function.boundary_settlements.clone();
    }
    if restored != *source.selected_plan() {
        return Err(DeadCompareError::ReplayMismatch);
    }
    Ok(ValidatedDeadCompare {
        receipt: DeadCompareReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
