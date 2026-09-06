//! Optimizer module role: executable entrance.
//! Ordered attached-Unit scalar-call legalization producer.

mod callee;
mod grammar;
mod nodes;
mod operations;
mod projection;

use super::shared::*;

/// Migration classification uses the legalization grammar, not trial emission.
pub(crate) fn is_ordered_scalar_call_unit(
    target: &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
    machine: semantic_vocabulary::MachineId,
) -> bool {
    let Some((index, function)) = target
        .target_operations()
        .functions
        .iter()
        .enumerate()
        .find(|(_, function)| function.machine == machine)
    else {
        return false;
    };
    let plan = target.optimized().plan();
    let unit = target.optimized().unit();
    let Some(abstracted) = plan
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    let Some(optimized) = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
    else {
        return false;
    };
    grammar::match_sequence(
        index,
        function,
        abstracted,
        optimized,
        target.target_operations(),
        plan,
        unit,
    )
    .is_ok()
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
