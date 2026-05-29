use omega_abstract_operations::AbstractOperationPlan;
use omega_instruction_selection::{InstructionSelectionInput, build_instruction_plan};

mod boundary;
mod input;
mod ownership;
mod semantics;
mod values;

pub use input::AbstractOperationLoweringInput;
use semantics::build_abstract_semantic_summary;

pub(crate) fn build_abstract_operation_plan(
    input: &AbstractOperationLoweringInput<'_>,
) -> AbstractOperationPlan {
    let mut plan = build_instruction_plan(&InstructionSelectionInput::from(input));
    plan.semantics = build_abstract_semantic_summary(input);
    plan
}
