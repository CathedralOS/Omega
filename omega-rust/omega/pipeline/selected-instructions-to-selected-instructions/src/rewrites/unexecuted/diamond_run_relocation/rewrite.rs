use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{DiamondRunRelocationError, ValidatedDiamondRunRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted run through its block's branch diamond: the
/// contiguous run the named `first_member` and `last_member` bound leaves
/// its own block's body as a single body — its members keeping their own
/// order — and takes the named `destination` instruction's position in
/// the join block the branch's arms alone feed, with the destination and
/// every later position keeping their relative order one run-width
/// later. Naming the join's terminator-carried instruction lands the run
/// at the body end. Admission has proven the crossed window independent —
/// no register or unit hazard between any member and any crossed
/// position, no edge-transport interference on the branch or arm edges,
/// no roster-carrying run sharing the window with a second memory actor,
/// no barrier or settlement whose observed state changes — so every
/// observer sees the same values, memory order, and boundary state it saw
/// before. Every other function, block, instruction, register, roster
/// row, call, settlement, and edge is retained, and replay independently
/// confirms that.
pub fn relocate_selected_run_through_diamond(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
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
    let mut transformed = source.selected_plan().clone();
    let run: Vec<_> = transformed.functions[function_index].blocks[admitted.block_index]
        .instructions
        .drain(admitted.first_index..=admitted.last_index)
        .collect();
    transformed.functions[function_index].blocks[admitted.target_index]
        .instructions
        .splice(admitted.landing_index..admitted.landing_index, run);
    super::validate_diamond_run_relocation(
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
