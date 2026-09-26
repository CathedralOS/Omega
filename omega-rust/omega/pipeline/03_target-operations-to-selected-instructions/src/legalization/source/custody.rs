use crate::{LegalizationError, LegalizationError as Error};
use abstract_operations_to_target_operations::target_operations::TargetOperationPlan;
use terminal_psi_to_abstract_operations::abstract_operations::AbstractOperationPlan;
use terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationUnit;

pub(super) fn validate_source_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<(), LegalizationError> {
    if crate::legalization::scalar_graph_input::validate_unit_custody(
        target,
        abstract_plan,
        unit,
        verified_input,
    )
    .is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || terminal_psi_to_abstract_operations::optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::custody());
    }
    Ok(())
}
