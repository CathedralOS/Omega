use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{BypassRelocationError, ValidatedBypassRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted member through its block's bypassed branch: the
/// named `member` leaves its own block's body and takes the named
/// `destination` instruction's position in the join block at least one
/// branch edge lands on directly, with the destination and every later
/// position keeping their relative order one slot later. Naming the
/// join's terminator-carried instruction lands the member at the body
/// end. Admission has proven the crossed window independent — no register
/// or unit hazard between the member and any crossed position, no
/// edge-transport interference on the branch or arm edges, no
/// roster-carrying member sharing the window with a second memory actor,
/// no barrier or settlement whose observed state changes — so every
/// observer sees the same values, memory order, and boundary state it saw
/// before. Every other function, block, instruction, register, roster
/// row, call, settlement, and edge is retained, and replay independently
/// confirms that.
pub fn relocate_selected_instruction_through_bypass(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedBypassRelocation, BypassRelocationError> {
    let admitted = admission::admit(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let mut transformed = source.selected_plan().clone();
    let member_instruction = transformed.functions[function_index].blocks[admitted.block_index]
        .instructions
        .remove(admitted.member_index);
    transformed.functions[function_index].blocks[admitted.target_index]
        .instructions
        .insert(admitted.landing_index, member_instruction);
    super::validate_bypass_relocation(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
        transformed,
    )
}
