//! Proposal of the in-block commuting member-against-run interchange:
//! admit the window the named member and run bound, then reorder the
//! window to the later side, the interior, and the earlier side — the
//! member-against-run interchange's own splice — and rewrite the window's
//! roster rows into the new execution order. Validation independently
//! re-derives the transform and restores the source by content.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{
    CommutingMemberRunInterchangeError, ValidatedCommutingMemberRunInterchange, admission,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;

/// Interchange one admitted member and run whose traded accesses commute:
/// the contiguous run the named `run_first` and `run_last` members bound
/// takes the member's side of their block's window, the named `member`
/// takes the run's side, and every instruction between them keeps its
/// relative order, shifted by one less than the run's length. The member
/// may sit before or after the run. Admission has proven the bounded
/// window independent — no register or unit hazard between any member and
/// any position outside its own side inside the window, every roster row
/// that newly trades order commuting with the position it crosses, no
/// barrier or interior settlement — so every observer sees the same
/// values, memory bytes, and boundary state it saw before. The memory
/// roster's window rows are rewritten into the new execution order so the
/// recorded accesses still appear in the order the program performs them;
/// every other instruction, register, roster row, call, settlement, and
/// edge is retained, and replay independently confirms that.
pub fn interchange_selected_commuting_member_and_run(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedCommutingMemberRunInterchange, CommutingMemberRunInterchangeError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        run_first,
        run_last,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let function = &mut transformed.functions[function_index];
    let instructions = &mut function.blocks[admitted.block_index].instructions;
    // The window holds earlier-side ++ interior ++ later-side, where the
    // earlier side is the member when it precedes the run and the run when
    // it follows; the interchange reorders it to
    // later-side ++ interior ++ earlier-side.
    let first = admitted.member_index.min(admitted.run_first_index);
    let last = admitted.member_index.max(admitted.run_last_index);
    let window: Vec<_> = instructions.drain(first..=last).collect();
    let run_len = admitted.run_last_index - admitted.run_first_index + 1;
    let member_earlier = admitted.member_index < admitted.run_first_index;
    let earlier_len = if member_earlier { 1 } else { run_len };
    let later_len = if member_earlier { run_len } else { 1 };
    let interior_len = window.len() - earlier_len - later_len;
    let rearranged: Vec<_> = window[earlier_len + interior_len..]
        .iter()
        .chain(&window[earlier_len..earlier_len + interior_len])
        .chain(&window[..earlier_len])
        .cloned()
        .collect();
    instructions.splice(first..first, rearranged);
    let window: Vec<SelectedInstructionId> = function.blocks[admitted.block_index].instructions
        [first..=last]
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    // The window's roster positions stay fixed while the rows that fill
    // them follow the new execution order: the later side's rows, the
    // interior's rows, then the earlier side's rows, each instruction's
    // own rows keeping their relative order.
    let positions = accesses::window_row_positions(function, &window);
    let ordered = accesses::rows_in_order(function, &window);
    debug_assert_eq!(positions.len(), ordered.len());
    for (position, access) in positions.into_iter().zip(ordered) {
        function.memory_accesses[position] = access;
    }
    super::validate_commuting_member_run_interchange(
        source,
        function_index,
        member,
        run_first,
        run_last,
        environment,
        budget,
        transformed,
    )
}
