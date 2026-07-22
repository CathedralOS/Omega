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
    let instruction_plan = build_instruction_plan(&InstructionSelectionInput::from(input));
    let mut semantics = build_abstract_semantic_summary(input);
    let _ = semantics
        .ownership
        .install_permission_realization_candidates(
            &instruction_plan.permission_realization_candidates,
            instruction_plan.code.instructions.len(),
        );
    semantics.boundaries.footprints = instruction_plan.semantics.boundaries.footprints;

    AbstractOperationPlan::with_roots(
        instruction_plan.code,
        semantics,
        instruction_plan.permission_realization_candidates,
    )
}
