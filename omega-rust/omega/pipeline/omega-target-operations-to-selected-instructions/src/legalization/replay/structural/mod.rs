//! Optimizer module role: executable entrance.
mod boundary_settlement;
mod call;
mod callee_contract;
mod contract;
mod operations;

use super::shared::*;
use super::validators::validate_structural_unit_form;
use contract::validate_replayed_contract;
use operations::replay_structural_operations;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_structural_unit_function(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed: &LegalizedStructuralUnitFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<usize, LegalizationError> {
    let validated = validate_structural_unit_form(target, abstracted, optimized, proposed.recipe)
        .ok_or(Error::NonCanonicalLegalizedPlan)?;
    validate_replayed_contract(
        function,
        target,
        abstracted,
        optimized,
        proposed,
        target_plan,
        abstract_plan,
        unit,
        &validated,
    )?;
    replay_structural_operations(
        function,
        proposed,
        &validated,
        &abstracted.entry_claims,
        target_plan,
        abstract_plan,
        unit,
    )?;
    Ok(0)
}
