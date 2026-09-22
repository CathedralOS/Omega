use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{MemberRunInterchangeError, ValidatedMemberRunInterchange, admission};
use crate::ValidatedSelectedAnalysis;

/// Interchange one admitted member and run: the contiguous run the named
/// `run_first` and `run_last` members bound takes the member's side of
/// their block's window, the named `member` takes the run's side, and
/// every instruction between them keeps its relative order, shifted by
/// one less than the run's length. The member may sit before or after the
/// run. Admission has proven the bounded window independent — no register
/// or unit hazard between any member and any position outside its own
/// side inside the window, no roster-carrying side sharing the window
/// with a second memory actor, no barrier or interior settlement — so
/// every observer sees the same values, memory order, and boundary state
/// it saw before. Every other function, block, instruction, register,
/// roster row, call, settlement, and edge is retained, and replay
/// independently confirms that.
pub fn interchange_selected_member_and_run(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    run_first: SelectedInstructionId,
    run_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedMemberRunInterchange, MemberRunInterchangeError> {
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
    let instructions =
        &mut transformed.functions[function_index].blocks[admitted.block_index].instructions;
    // The window holds earlier-side ++ interior ++ later-side, where the
    // earlier side is the member when it precedes the run and the run
    // when it follows; the interchange reorders it to
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
    super::validate_member_run_interchange(
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
