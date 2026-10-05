use crate::{LegalizationError, LegalizationError as Error};
use abstract_operations::AbstractOperationPlan;
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;

pub(super) fn validate_source_custody(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    if target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
    {
        return Err(Error::custody());
    }
    Ok(())
}
