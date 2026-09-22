use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{ForkRunRelocationError, ValidatedForkRunRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted run through its block's branch fork: the
/// contiguous run the named `first_member` and `last_member` bound leaves
/// its own block's body as a single body — its members keeping their own
/// order — and takes the named `destination` instruction's position in
/// the arm the branch's plain edges alone feed, with the destination and
/// every later position keeping their relative order one run-width later.
/// Naming the arm's terminator-carried instruction lands the run at the
/// body end. Admission has proven both sides of the conditional move —
/// the crossed window independent (no register or unit hazard between any
/// member and any crossed position, no landing-edge transport
/// interference, no barrier or settlement whose observed state changes)
/// and every member-written location dead on every path the run no longer
/// executes on — so every observer sees the same values, memory order,
/// and boundary state it saw before. Every other function, block,
/// instruction, register, roster row, call, settlement, and edge is
/// retained, and replay independently confirms that.
pub fn relocate_selected_run_into_arm(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedForkRunRelocation, ForkRunRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let run: Vec<_> = transformed.functions[function_index].blocks[admitted.block_index]
        .instructions
        .drain(admitted.first_index..=admitted.last_index)
        .collect();
    transformed.functions[function_index].blocks[admitted.target_index]
        .instructions
        .splice(admitted.landing_index..admitted.landing_index, run);
    super::validate_fork_run_relocation(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
        transformed,
    )
}
