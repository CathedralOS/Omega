//! Optimizer module role: executable entrance.
//! Ordered attached-Unit scalar-call legalization producer.

mod callee;
mod grammar;
mod nodes;
mod operations;
mod projection;

use super::shared::*;

pub(super) fn derive_source_scalar_call_unit_function(
    function: usize,
    target_function: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedScalarCallUnitFunction, LegalizationError> {
    let matched = grammar::match_sequence(
        function,
        target_function,
        abstracted,
        optimized,
        target,
        abstract_plan,
        unit,
    )?;
    projection::project(function, matched)
}
