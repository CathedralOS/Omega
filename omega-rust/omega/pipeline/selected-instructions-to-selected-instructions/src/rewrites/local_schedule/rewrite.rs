use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{LocalScheduleError, ValidatedLocalSchedule, admission};
use crate::ValidatedSelectedAnalysis;

/// Interchange one admitted pair: the named `later` instruction takes the
/// named `earlier` instruction's position inside their block and the earlier
/// instruction takes the later one's, leaving every instruction between them
/// in place. Admission has proven the bounded window independent — no
/// register or unit hazard between either member and any crossed position,
/// at most one roster-carrying memory actor in the window, no barrier or
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
        .swap(admitted.earlier_index, admitted.later_index);
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
