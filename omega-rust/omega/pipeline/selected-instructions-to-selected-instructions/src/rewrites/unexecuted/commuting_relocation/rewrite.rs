//! Proposal of the in-block commuting relocation: admit the window the
//! named member and destination bound, then rotate the member onto the
//! destination's position — the crossed run shifting one slot toward the
//! member's vacated index — and rewrite the window's roster rows into the
//! new execution order. Validation independently re-derives the transform
//! and restores the source by content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{CommutingRelocationError, ValidatedCommutingRelocation, admission};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;

/// Relocate one admitted member whose traded accesses commute: the named
/// `member` instruction takes the named `destination` instruction's
/// position inside their block and the whole crossed run — destination
/// included — shifts one slot toward the member's vacated position.
/// Admission has proven the bounded window independent — no register or
/// unit hazard between the member and any crossed position, every roster
/// row that newly trades order commuting with the position it crosses, no
/// barrier or interior settlement — so every observer sees the same
/// values, memory bytes, and boundary state it saw before. The memory
/// roster's window rows are rewritten into the new execution order so the
/// recorded accesses still appear in the order the program performs them;
/// every other instruction, register, roster row, call, settlement, and
/// edge is retained, and replay independently confirms that.
pub fn relocate_selected_commuting_member(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCommutingRelocation, CommutingRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    let instructions = &mut function.blocks[admitted.block_index].instructions;
    let first = admitted.member_index.min(admitted.destination_index);
    let last = admitted.member_index.max(admitted.destination_index);
    let moved = instructions.remove(admitted.member_index);
    instructions.insert(admitted.destination_index, moved);
    let window: Vec<SelectedInstructionId> = function.blocks[admitted.block_index].instructions
        [first..=last]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    // The window's roster positions stay fixed while the rows that fill
    // them follow the new execution order: when the member moved down the
    // crossed run's rows lead and the member's close the window, and the
    // reverse when it moved up — each instruction's own rows keeping their
    // relative order.
    let positions = accesses::window_row_positions(function, &window);
    let ordered = accesses::rows_in_order(function, &window);
    debug_assert_eq!(positions.len(), ordered.len());
    for (position, access) in positions.into_iter().zip(ordered) {
        function.memory_accesses[position] = access;
    }
    super::validate_commuting_relocation(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
        transformed,
    )
}
