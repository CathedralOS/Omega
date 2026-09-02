//! Optimizer module role: executable entrance.
//! Independent replay for the closed attached-Unit scalar-call family.

mod callee;
mod contract;
mod grammar;
mod operations;

use super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_scalar_call_unit_function(
    function: usize,
    target_function: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    proposed: &LegalizedScalarCallUnitFunction,
) -> Result<usize, LegalizationError> {
    contract::replay(
        function,
        target_function,
        abstracted,
        optimized,
        target,
        abstract_plan,
        unit,
        proposed_plan,
        proposed,
    )?;
    Ok(0)
}
