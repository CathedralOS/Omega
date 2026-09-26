use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{ScheduledRelocationError, ValidatedScheduledRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted run along its derived window: the named
/// `members` — a contiguous run in their own block's body — leave it and
/// take the `destination` instruction's position in the destination's
/// block, with the destination and every later position keeping their
/// relative order `members.len()` slots later. Naming a terminator-carried
/// instruction lands the run at the body end. Admission has proven the
/// whole derived window — the crossed positions and edges on every path
/// between the run and the destination independent under the hazard
/// audit, every member-written location dead on every traversal the run
/// no longer executes on, and no inflow gaining the run — so every
/// observer sees the same values, memory order, and boundary state it saw
/// before. Every other function, block, instruction, register, roster row,
/// call, settlement, and edge is retained, and replay independently
/// confirms that.
pub fn relocate_scheduled_run(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedScheduledRelocation, ScheduledRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        members,
        destination,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let vacated: Vec<_> = transformed.functions[function_index].blocks[admitted.block_index]
        .instructions
        .drain(admitted.member_first..=admitted.member_last)
        .collect();
    transformed.functions[function_index].blocks[admitted.target_index]
        .instructions
        .splice(admitted.landing_index..admitted.landing_index, vacated);
    super::validate_scheduled_relocation(
        source,
        function_index,
        members,
        destination,
        environment,
        budget,
        transformed,
    )
}
