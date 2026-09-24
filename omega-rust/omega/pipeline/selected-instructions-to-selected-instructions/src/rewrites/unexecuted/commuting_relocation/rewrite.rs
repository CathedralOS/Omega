//! Proposal of the in-block commuting run relocation: admit the window the
//! named run and destination bound, then rotate the run onto the
//! destination's position — the crossed positions shifting one run-width
//! toward the run's vacated span — and rewrite the window's roster rows
//! into the new execution order. Validation independently re-derives the
//! transform and restores the source by content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{CommutingRelocationError, ValidatedCommutingRelocation, admission};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;

/// Relocate one admitted run whose traded accesses commute: the contiguous
/// run the named `first_member` and `last_member` bound takes the named
/// `destination` instruction's position inside their block and the whole
/// crossed run — the destination included — shifts one run-width toward
/// the run's vacated span. Toward an earlier destination the run lands on
/// the window's leading edge; toward a later destination it lands on the
/// trailing edge. Admission has proven the bounded window independent — no
/// register or unit hazard between any member and any crossed position,
/// every roster row that newly trades order commuting with the position it
/// crosses, no barrier or interior settlement — so every observer sees the
/// same values, memory bytes, and boundary state it saw before. The memory
/// roster's window rows are rewritten into the new execution order so the
/// recorded accesses still appear in the order the program performs them;
/// every other instruction, register, roster row, call, settlement, and
/// edge is retained, and replay independently confirms that.
pub fn relocate_selected_commuting_members(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCommutingRelocation, CommutingRelocationError> {
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
    let function = &mut transformed.functions[function_index];
    let instructions = &mut function.blocks[admitted.block_index].instructions;
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
    let first = admitted.first_index.min(admitted.destination_index);
    let last = admitted.last_index.max(admitted.destination_index);
    let window: Vec<SelectedInstructionId> = function.blocks[admitted.block_index].instructions
        [first..=last]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    // The window's roster positions stay fixed while the rows that fill
    // them follow the new execution order: when the run moved down the
    // crossed positions' rows lead and the members' close the window, and
    // the reverse when it moved up — each instruction's own rows keeping
    // their relative order.
    let positions = accesses::window_row_positions(function, &window);
    let ordered = accesses::rows_in_order(function, &window);
    debug_assert_eq!(positions.len(), ordered.len());
    for (position, access) in positions.into_iter().zip(ordered) {
        function.memory_accesses[position] = access;
    }
    super::validate_commuting_relocation(
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
