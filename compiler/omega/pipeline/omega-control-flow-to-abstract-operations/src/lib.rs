use omega_abstract_operations::AbstractOperationPlan;

mod lowering;

pub use lowering::AbstractOperationLoweringInput;

pub fn build_abstract_operation_plan(
    input: &AbstractOperationLoweringInput<'_>,
) -> AbstractOperationPlan {
    lowering::build_abstract_operation_plan(input)
}
