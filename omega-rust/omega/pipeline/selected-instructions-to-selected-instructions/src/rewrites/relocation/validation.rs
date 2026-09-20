use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    MemberRunRelocationError, MemberRunRelocationReceipt, ValidatedMemberRunRelocation, admission,
};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: admission re-derives the
/// admitted run, window and landing index from the source, the proposal
/// must place exactly the run's members on the landing index in the
/// destination block in their original order, and moving the run back
/// must restore the complete source by content — every crossed
/// instruction, every other block and instruction, register, roster row,
/// call, settlement, and edge included.
pub fn validate_member_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedMemberRunRelocation, MemberRunRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = admitted.run_end - admitted.run_start + 1;
    // A forward in-block move counts its landing index in source order
    // and puts the run's last member on it; the run's transformed start
    // sits one width less the named slot behind.
    let insert_index = if admitted.run_block == admitted.destination_block
        && admitted.landing_index > admitted.run_end
    {
        admitted.landing_index - run_len + 1
    } else {
        admitted.landing_index
    };
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(admitted.destination_block))
        .and_then(|block| block.instructions.get(insert_index..insert_index + run_len))
        != Some(
            &admitted.function.blocks[admitted.run_block].instructions
                [admitted.run_start..=admitted.run_end],
        )
    {
        return Err(MemberRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[admitted.destination_block]
        .instructions
        .drain(insert_index..insert_index + run_len)
        .collect();
    restored.functions[function_index].blocks[admitted.run_block]
        .instructions
        .splice(admitted.run_start..admitted.run_start, run);
    if restored != *source.selected_plan() {
        return Err(MemberRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedMemberRunRelocation {
        receipt: MemberRunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
