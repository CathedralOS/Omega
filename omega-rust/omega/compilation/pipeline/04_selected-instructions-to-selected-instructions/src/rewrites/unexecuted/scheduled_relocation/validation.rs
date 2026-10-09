use crate::rewrites::sealed;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstructionId, SelectedInstructionPlan};

use super::{ScheduledRelocationError, ScheduledRelocationReceipt, ValidatedScheduledRelocation};
use crate::ValidatedSelectedAnalysis;

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, derived region, and landing index from the source
/// without the producer's admission routine, the proposal must place
/// exactly the run's instructions, contiguous and in order, on the
/// landing index in the destination block, and moving them back must
/// restore the complete source by content — every crossed instruction,
/// every other block and instruction, register, roster row, call,
/// settlement, and edge included.
/// Independently consume the proposed program: the validator re-derives
/// the admitted run, derived region, and landing index from the source
/// without the producer's admission routine, the proposal must place
/// exactly the run's instructions, contiguous and in order, on the
/// landing index in the destination block, and moving them back must
/// restore the complete source by content — every crossed instruction,
/// every other block and instruction, register, roster row, call,
/// settlement, and edge included.
pub(crate) fn validate_scheduled_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedScheduledRelocation, ScheduledRelocationError> {
    let reconstructed = super::reconstruct(
        source,
        function_index,
        members,
        destination,
        environment,
        budget,
    )?;
    let span = reconstructed.member_last - reconstructed.member_first + 1;
    let source_run: Vec<&_> = reconstructed.function.blocks[reconstructed.block_index].instructions
        [reconstructed.member_first..=reconstructed.member_last]
        .iter()
        .collect();
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(ScheduledRelocationError::ReplayMismatch)?;
    let proposed_target = proposed_function
        .blocks
        .get(reconstructed.target_index)
        .ok_or(ScheduledRelocationError::ReplayMismatch)?;
    let landed = proposed_target
        .instructions
        .get(reconstructed.landing_index..);
    if landed.map(|tail| tail.len() < span).unwrap_or(true)
        || source_run.iter().enumerate().any(|(offset, member)| {
            proposed_target.instructions[reconstructed.landing_index + offset] != **member
        })
    {
        return Err(ScheduledRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let vacated: Vec<_> = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .drain(reconstructed.landing_index..reconstructed.landing_index + span)
        .collect();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(
            reconstructed.member_first..reconstructed.member_first,
            vacated,
        );
    if restored != *source.selected_plan() {
        return Err(ScheduledRelocationError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedScheduledRelocation,
        ScheduledRelocationReceipt,
        source,
        proposed
    ))
}
