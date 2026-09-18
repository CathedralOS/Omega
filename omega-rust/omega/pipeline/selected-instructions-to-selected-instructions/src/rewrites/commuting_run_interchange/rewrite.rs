//! Proposal of the in-block commuting run interchange: admit the window
//! the two named runs bound, then reorder the window to the later run, the
//! interior, and the earlier run — the run interchange's own splice — and
//! rewrite the window's roster rows into the new execution order.
//! Validation independently re-derives the transform and restores the
//! source by content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{CommutingRunInterchangeError, ValidatedCommutingRunInterchange, admission};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;

/// Interchange two admitted runs whose traded accesses commute: the
/// contiguous run the named `later_first` and `later_last` members bound
/// takes the earlier run's position inside their block, the earlier run
/// takes the later run's position, and every instruction between them
/// keeps its relative order, shifted by the difference of the runs'
/// lengths. Admission has proven the bounded window independent — no
/// register or unit hazard between any member and any position outside its
/// own run inside the window, every roster row that newly trades order
/// commuting with the position it crosses, no barrier or interior
/// settlement — so every observer sees the same values, memory bytes, and
/// boundary state it saw before. The memory roster's window rows are
/// rewritten into the new execution order so the recorded accesses still
/// appear in the order the program performs them; every other instruction,
/// register, roster row, call, settlement, and edge is retained, and
/// replay independently confirms that.
pub fn interchange_selected_commuting_runs(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCommutingRunInterchange, CommutingRunInterchangeError> {
    let admitted = admission::admit(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    let instructions = &mut function.blocks[admitted.block_index].instructions;
    // The window holds earlier-run ++ interior ++ later-run; the
    // interchange reorders it to later-run ++ interior ++ earlier-run.
    let window: Vec<_> = instructions
        .drain(admitted.earlier_first_index..=admitted.later_last_index)
        .collect();
    let earlier_len = admitted.earlier_last_index - admitted.earlier_first_index + 1;
    let later_len = admitted.later_last_index - admitted.later_first_index + 1;
    let interior_len = window.len() - earlier_len - later_len;
    let rearranged: Vec<_> = window[earlier_len + interior_len..]
        .iter()
        .chain(&window[earlier_len..earlier_len + interior_len])
        .chain(&window[..earlier_len])
        .cloned()
        .collect();
    instructions.splice(
        admitted.earlier_first_index..admitted.earlier_first_index,
        rearranged,
    );
    let window: Vec<SelectedInstructionId> = function.blocks[admitted.block_index].instructions
        [admitted.earlier_first_index..=admitted.later_last_index]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    // The window's roster positions stay fixed while the rows that fill
    // them follow the new execution order: the later run's rows, the
    // interior's rows, then the earlier run's rows, each instruction's own
    // rows keeping their relative order.
    let positions = accesses::window_row_positions(function, &window);
    let ordered = accesses::rows_in_order(function, &window);
    debug_assert_eq!(positions.len(), ordered.len());
    for (position, access) in positions.into_iter().zip(ordered) {
        function.memory_accesses[position] = access;
    }
    super::validate_commuting_run_interchange(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
        transformed,
    )
}
