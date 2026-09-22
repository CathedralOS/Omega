use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{TriangleRelocationError, ValidatedTriangleRelocation, admission};
use crate::ValidatedSelectedAnalysis;

/// Relocate one admitted member out of its converging join: the named
/// `member` leaves the join block's body, crosses the arm, its `Jump`,
/// both branch edges — the bypass included — and the branch between the
/// join and the one fork head the region descends from, and takes the
/// named `destination` instruction's position in the head's body, with
/// the destination and every later position keeping their relative order
/// one slot later. Naming the head's terminator-carried instruction lands
/// the member at the body end. Admission has proven the crossed window
/// independent — no register or unit hazard between the member and any
/// crossed position, no edge-transport interference on the branch or arm
/// edges, no roster-carrying member sharing the window with a second
/// memory actor, no barrier or settlement whose observed state changes —
/// and proven the member's supply total: every path into the join
/// traversed the head. Every other function, block, instruction,
/// register, roster row, call, settlement, and edge is retained, and
/// replay independently confirms that.
pub fn relocate_selected_instruction_out_of_triangle(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedTriangleRelocation, TriangleRelocationError> {
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
    super::validate_triangle_relocation(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
        transformed,
    )
}
