use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{LocalRelocationError, ValidatedLocalRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted member: the named `member` instruction takes the
/// named `destination` instruction's position inside their block and the
/// whole crossed run — destination included — shifts one slot toward the
/// member's vacated position. Admission has proven the bounded window
/// independent — no register or unit hazard between the member and any
/// crossed position, no roster-carrying member sharing the window with a
/// second memory actor, no barrier or interior settlement — so every
/// observer sees the same values, memory order, and boundary state it saw
/// before. Every other function, block, instruction, register, roster row,
/// call, settlement, and edge is retained, and replay independently
/// confirms that.
pub fn relocate_selected_instruction(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedLocalRelocation, LocalRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let instructions =
        &mut transformed.functions[function_index].blocks[admitted.block_index].instructions;
    let moved = instructions.remove(admitted.member_index);
    instructions.insert(admitted.destination_index, moved);
    super::validate_local_relocation(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
        transformed,
    )
}
