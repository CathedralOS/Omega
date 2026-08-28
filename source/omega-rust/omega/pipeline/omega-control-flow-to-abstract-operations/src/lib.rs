use omega_abstract_operations::AbstractOperationPlan;

mod lowering;

pub use lowering::AbstractOperationLoweringInput;

pub fn build_abstract_operation_plan(
    input: &AbstractOperationLoweringInput<'_>,
) -> Result<AbstractOperationPlan, psi_diagnostics::Diagnostic> {
    lowering::build_abstract_operation_plan(input)
}
