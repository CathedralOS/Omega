//! Optimizer module role: executable entrance.
//! Ordered attached-Unit scalar-call legalization producer.

mod callee;
mod grammar;
mod nodes;
mod operations;
mod projection;

use super::shared::*;

/// The producer and migration classifier share the same borrowed input grammar.
#[allow(clippy::too_many_arguments)]
pub(super) fn matches_input(
    index: usize,
    function: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    target: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> bool {
    grammar::match_sequence(index, function, abstracted, optimized, target, plan, unit).is_ok()
}

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
