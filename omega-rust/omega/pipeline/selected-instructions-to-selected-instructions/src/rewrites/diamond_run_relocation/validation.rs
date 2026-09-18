use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    DiamondRunRelocationError, DiamondRunRelocationReceipt, ValidatedDiamondRunRelocation,
    admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted run, diamond, and landing index from the source, the proposal
/// must place exactly the run's members on the landing index in the join
/// block in their original order, and moving the run back must restore
/// the complete source by content — every crossed instruction, every
/// other block and instruction, register, roster row, call, settlement,
/// and edge included.
pub fn validate_diamond_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDiamondRunRelocation, DiamondRunRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = admitted.last_index - admitted.first_index + 1;
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.target_index))
        .and_then(|block| {
            block
                .instructions
                .get(admitted.landing_index..admitted.landing_index + run_len)
        })
        != Some(
            &admitted.function.blocks[admitted.block_index].instructions
                [admitted.first_index..=admitted.last_index],
        )
    {
        return Err(DiamondRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[admitted.target_index]
        .instructions
        .drain(admitted.landing_index..admitted.landing_index + run_len)
        .collect();
    restored.functions[function_index].blocks[admitted.block_index]
        .instructions
        .splice(admitted.first_index..admitted.first_index, run);
    if restored != *source.selected_plan() {
        return Err(DiamondRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedDiamondRunRelocation {
        receipt: DiamondRunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
