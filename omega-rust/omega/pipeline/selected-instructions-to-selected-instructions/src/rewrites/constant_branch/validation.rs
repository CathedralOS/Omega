use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{ConstantBranchError, ConstantBranchReceipt, ValidatedConstantBranch, admission};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted branch and re-resolves its flag units' reaching compare from
/// the source, the proposed terminator must equal the reconstructed `Jump`
/// — rebuilt instruction and decided successor alike — and reinserting the
/// source terminator must restore the complete source by content: every
/// other instruction, register, roster row, call, settlement, and function
/// included.
pub fn validate_constant_branch_fold(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    branch: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConstantBranch, ConstantBranchError> {
    let admitted = admission::admit(source, function_index, branch, environment, budget)?;
    let function = proposed
        .functions
        .get(function_index)
        .ok_or(ConstantBranchError::ReplayMismatch)?;
    if function
        .blocks
        .get(admitted.block_index)
        .map(|block| &block.terminator)
        != Some(&admission::rewritten(&admitted))
    {
        return Err(ConstantBranchError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[admitted.block_index].terminator =
        source.selected_plan().functions[function_index].blocks[admitted.block_index]
            .terminator
            .clone();
    if restored != *source.selected_plan() {
        return Err(ConstantBranchError::ReplayMismatch);
    }
    Ok(ValidatedConstantBranch {
        receipt: ConstantBranchReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
