use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    PredecessorRelocationError, PredecessorRelocationReceipt, ValidatedPredecessorRelocation,
    admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted member, edge, and landing index from the source, the proposal
/// must place exactly the member's instruction on the landing index in
/// the predecessor block, and moving it back must restore the complete
/// source by content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub fn validate_predecessor_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedPredecessorRelocation, PredecessorRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_member =
        &admitted.function.blocks[admitted.block_index].instructions[admitted.member_index];
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.target_index))
        .and_then(|block| block.instructions.get(admitted.landing_index))
        != Some(source_member)
    {
        return Err(PredecessorRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[admitted.target_index]
        .instructions
        .remove(admitted.landing_index);
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .insert(admitted.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(PredecessorRelocationError::ReplayMismatch);
    }
    Ok(ValidatedPredecessorRelocation {
        receipt: PredecessorRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
