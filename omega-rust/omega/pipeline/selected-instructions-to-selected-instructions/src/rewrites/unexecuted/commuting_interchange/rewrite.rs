//! Proposal of the in-block commuting pair interchange: admit the window
//! the named pair bounds, then swap the named instructions inside the
//! block — every position between them keeps its index — and rewrite the
//! window's roster rows into the new execution order. Validation
//! independently re-derives the transform and restores the source by
//! content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{CommutingInterchangeError, ValidatedCommutingInterchange, admission};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;

/// Interchange one admitted commuting pair: the named `later` instruction
/// takes the named `earlier` instruction's position inside their block and
/// the earlier instruction takes the later one's, leaving every instruction
/// between them in place. Admission has proven the bounded window
/// independent — no register or unit hazard between either member and any
/// crossed position, every roster row that newly trades order commuting
/// with the position it crosses, no barrier or interior settlement — so
/// every observer sees the same values, memory bytes, and boundary state it
/// saw before. The memory roster's window rows are rewritten into the new
/// execution order so the recorded accesses still appear in the order the
/// program performs them; every other instruction, register, roster row,
/// call, settlement, and edge is retained, and replay independently
/// confirms that.
pub fn interchange_selected_commuting_pair(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCommutingInterchange, CommutingInterchangeError> {
    let admitted = admission::admit(source, function_index, earlier, later, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    function.blocks[admitted.block_index]
        .instructions
        .swap(admitted.earlier_index, admitted.later_index);
    let window: Vec<SelectedInstructionId> = function.blocks[admitted.block_index].instructions
        [admitted.earlier_index..=admitted.later_index]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    // The window's roster positions stay fixed while the rows that fill
    // them follow the new execution order: the later member's rows, the
    // interior's rows, then the earlier member's rows, each instruction's
    // own rows keeping their relative order.
    let positions = accesses::window_row_positions(function, &window);
    let ordered = accesses::rows_in_order(function, &window);
    debug_assert_eq!(positions.len(), ordered.len());
    for (position, access) in positions.into_iter().zip(ordered) {
        function.memory_accesses[position] = access;
    }
    super::validate_commuting_interchange(
        source,
        function_index,
        earlier,
        later,
        environment,
        budget,
        transformed,
    )
}
