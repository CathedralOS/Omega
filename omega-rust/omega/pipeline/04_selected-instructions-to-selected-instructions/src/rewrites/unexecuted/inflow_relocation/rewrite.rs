use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{InflowRelocationError, ValidatedInflowRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted member onto the inflow: the named `member`
/// leaves its own block's body — a join at least one other predecessor's
/// edge also feeds — crosses the one inflow edge the named `destination`'s
/// block ends in, and takes the destination instruction's position inside
/// that predecessor, with the destination and every later position
/// keeping their relative order one slot later. Naming the inflow's
/// terminator-carried instruction lands the member at the body end.
/// Admission has proven both sides of the removing move — the crossed
/// window independent (no register or unit hazard between the member and
/// any crossed position, no crossed-edge transport interference, no
/// barrier or settlement whose observed state changes) and every
/// member-written location dead on the arrivals through the other
/// inflows, where the vacated index publishes them — so every observer
/// sees the same values, memory order, and boundary state it saw before.
/// Every other function, block, instruction, register, roster row, call,
/// settlement, and edge is retained, and replay independently confirms
/// that.
pub fn relocate_selected_instruction_onto_inflow(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedInflowRelocation, InflowRelocationError> {
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
    super::validate_inflow_relocation(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
        transformed,
    )
}
