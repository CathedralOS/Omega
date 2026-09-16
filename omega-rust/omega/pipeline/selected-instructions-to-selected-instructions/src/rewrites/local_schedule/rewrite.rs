use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{LocalScheduleError, ValidatedLocalSchedule, admission};
use crate::ValidatedSelectedAnalysis;

/// Interchange one admitted adjacent pair: the named `later` instruction is
/// scheduled ahead of the named `earlier` instruction inside their block.
/// Admission has proven the pair independent — no register or unit hazard in
/// either direction, at most one roster-carrying memory actor, no barrier or
/// interior settlement — so every observer sees the same values, memory
/// order, and boundary state it saw before. Every other function, block,
/// instruction, register, roster row, call, settlement, and edge is
/// retained, and replay independently confirms that.
pub fn schedule_selected_pair(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedLocalSchedule, LocalScheduleError> {
    let admitted = admission::admit(source, function_index, earlier, later, environment, budget)?;
    let mut transformed = source.selected_plan().clone();
    transformed.functions[function_index].blocks[admitted.block_index]
        .instructions
        .swap(admitted.earlier_index, admitted.earlier_index + 1);
    super::validate_local_schedule(
        source,
        function_index,
        earlier,
        later,
        environment,
        budget,
        transformed,
    )
}
