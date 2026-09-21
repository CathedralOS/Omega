use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{MemberRunRelocationError, ValidatedMemberRunRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted member run to the named destination: the
/// contiguous run `first_member`..`last_member` bound in its block's body
/// leaves that body as a single body — its members keeping their own
/// order — and takes the named `destination` instruction's position in
/// whichever block holds it, with the destination and every later
/// position keeping their relative order one run-width later. Naming a
/// block's terminator-carried instruction lands the run at that block's
/// body end, and naming a position inside the run's own block is the
/// in-block move. Admission has proven the window independent — the move
/// changes no traversal, and no hazard, transport, memory-ordering or
/// settlement boundary observes a changed executed prefix — so every
/// observer sees the same values, memory order, and boundary state it saw
/// before. Every other function, block, instruction, register, roster
/// row, call, settlement, and edge is retained, and replay independently
/// confirms that.
pub fn relocate_selected_member_run(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
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
    let mut transformed = source.selected_plan().clone();
    let run: Vec<_> = transformed.functions[function_index].blocks[admitted.run_block]
        .instructions
        .drain(admitted.run_start..=admitted.run_end)
        .collect();
    // The run takes the named destination position in the destination
    // block's transformed stream. A forward in-block move counts the
    // landing index in source order and puts the run's last member on
    // it, so vacating the run shifts the insert point back one width
    // less the named slot the run itself occupies.
    let insert_index = if admitted.run_block == admitted.destination_block
        && admitted.landing_index > admitted.run_end
    {
        admitted.landing_index - run.len() + 1
    } else {
        admitted.landing_index
    };
    transformed.functions[function_index].blocks[admitted.destination_block]
        .instructions
        .splice(insert_index..insert_index, run);
    super::validate_member_run_relocation(
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
