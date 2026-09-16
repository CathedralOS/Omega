use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{RunRelocationError, ValidatedRunRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted run: the contiguous run the named `first_member`
/// and `last_member` bound takes the named `destination` instruction's
/// position inside their block and the whole crossed run — the destination
/// included — shifts one run-width toward the run's vacated span. Toward an
/// earlier destination the run lands on the window's leading edge; toward a
/// later destination it lands on the trailing edge. Admission has proven
/// the bounded window independent — no register or unit hazard between any
/// member and any crossed position, no roster-carrying run sharing the
/// window with a second memory actor, no barrier or interior settlement —
/// so every observer sees the same values, memory order, and boundary state
/// it saw before. Every other function, block, instruction, register,
/// roster row, call, settlement, and edge is retained, and replay
/// independently confirms that.
pub fn relocate_selected_run(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRunRelocation, RunRelocationError> {
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
    let instructions =
        &mut transformed.functions[function_index].blocks[admitted.block_index].instructions;
    let run: Vec<_> = instructions
        .drain(admitted.first_index..=admitted.last_index)
        .collect();
    // The run lands on the destination's edge of the window: at the
    // destination's own index toward an earlier destination, and with its
    // last member on the destination's index toward a later one — where
    // the drain has already shifted the destination one run-width toward
    // the vacated span.
    let insert_index = if admitted.destination_index < admitted.first_index {
        admitted.destination_index
    } else {
        admitted.destination_index + 1 - run.len()
    };
    instructions.splice(insert_index..insert_index, run);
    super::validate_run_relocation(
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
