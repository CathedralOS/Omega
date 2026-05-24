use omega_abstract_operations::AbstractOperationPlan;
use omega_instruction_selection::{InstructionSelectionInput, build_instruction_plan};

pub fn build_abstract_operation_plan(
    input: &InstructionSelectionInput<'_>,
) -> AbstractOperationPlan {
    build_instruction_plan(input).into()
}
