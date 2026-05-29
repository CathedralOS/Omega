use omega_abstract_operations::AbstractOperationPlan;
use omega_instruction_selection::{InstructionSelectionInput, build_instruction_plan};

mod boundary;
mod input;
mod ownership;
mod values;

use boundary::build_abstract_boundary_summary;
pub use input::AbstractOperationLoweringInput;
use ownership::build_abstract_ownership_summary;
use values::build_abstract_value_summary;

pub(crate) fn build_abstract_operation_plan(
    input: &AbstractOperationLoweringInput<'_>,
) -> AbstractOperationPlan {
    let mut plan = build_instruction_plan(&InstructionSelectionInput::from(input));
    plan.semantics.boundary_edges =
        build_abstract_boundary_summary(input.control_flow, input.host_calls);
    plan.semantics.ownership = build_abstract_ownership_summary(input.control_flow);
    plan.semantics.values = build_abstract_value_summary(input.control_flow);
    plan
}
