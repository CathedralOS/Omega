use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{LiteralMinuendError, LiteralMinuendReceipt, ValidatedLiteralMinuend, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted pair and re-audits the compare's reachable flag consumers from
/// the source, the rewritten instruction must equal the reconstructed
/// compare form, and reinserting the source compare must restore the
/// complete source by content — every other instruction, register, roster
/// row, call, settlement, and function included.
pub fn validate_literal_minuend_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    compare: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedLiteralMinuend, LiteralMinuendError> {
    let admitted = admission::admit(source, function_index, compare, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(LiteralMinuendError::ReplayMismatch)?;
    if function
        .blocks
        .get(admitted.block_index)
        .and_then(|block| block.instructions.get(admitted.compare_index))
        != Some(&admission::rewritten(&admitted))
    {
        return Err(LiteralMinuendError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index].instructions
        [admitted.compare_index] =
        admitted.function.blocks[admitted.block_index].instructions[admitted.compare_index].clone();
    if restored != *source.selected_plan() {
        return Err(LiteralMinuendError::ReplayMismatch);
    }
    Ok(ValidatedLiteralMinuend {
        receipt: LiteralMinuendReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
