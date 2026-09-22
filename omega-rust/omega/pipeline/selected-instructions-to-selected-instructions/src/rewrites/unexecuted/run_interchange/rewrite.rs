use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{RunInterchangeError, ValidatedRunInterchange, admission};
use crate::ValidatedSelectedAnalysis;

/// Interchange two admitted runs: the contiguous run the named
/// `later_first` and `later_last` members bound takes the earlier run's
/// position inside their block, the earlier run takes the later run's
/// position, and every instruction between them keeps its relative order,
/// shifted by the difference of the runs' lengths. Admission has proven
/// the bounded window independent — no register or unit hazard between any
/// member and any position outside its own run inside the window, no
/// roster-carrying run sharing the window with a second memory actor, no
/// barrier or interior settlement — so every observer sees the same
/// values, memory order, and boundary state it saw before. Every other
/// function, block, instruction, register, roster row, call, settlement,
/// and edge is retained, and replay independently confirms that.
pub fn interchange_selected_runs(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedRunInterchange, RunInterchangeError> {
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
    let instructions =
        &mut transformed.functions[function_index].blocks[admitted.block_index].instructions;
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
    super::validate_run_interchange(
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
