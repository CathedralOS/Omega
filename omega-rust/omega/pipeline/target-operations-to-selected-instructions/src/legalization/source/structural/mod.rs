//! Optimizer module role: executable entrance.
mod boundary_settlement;
mod call;
mod callee_contract;
mod contract;
mod graph;
mod input;
mod operations;

#[cfg(test)]
pub(super) use input::accepts as accepts_publication_input;

use super::matchers::MatchedStructuralUnitForm;
use super::shared::*;
use contract::validate_and_derive_parameters;
use operations::derive_structural_operations;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_source_structural_unit_function(
    function: usize,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    matched: MatchedStructuralUnitForm<'_>,
) -> Result<legalized_operations::LegalizedScalarFunction, LegalizationError> {
    let parameters = validate_and_derive_parameters(
        function,
        target,
        abstracted,
        optimized,
        target_plan,
        abstract_plan,
        unit,
        &matched,
    )?;
    let operations = derive_structural_operations(
        function,
        &matched,
        &parameters,
        &abstracted.entry_claims,
        target_plan,
        abstract_plan,
        unit,
    )?;
    Ok(graph::assemble(
        target, abstracted, optimized, &matched, parameters, operations,
    ))
}
