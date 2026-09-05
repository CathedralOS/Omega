//! Optimizer module role: executable entrance. Independent replay of a proposed legal-operation projection.

pub(in crate::legalization) mod conditions;
mod custody;
mod functions;
mod leaf;
mod ordinary_roster;
mod scalar_call_unit;
mod shared;
mod structural;
mod validators;

use crate::legalization::projected_structural_call_return;
use functions::{replay_function, replay_unit_function};
use scalar_call_unit::replay_scalar_call_unit_function;
use shared::*;
use structural::replay_structural_unit_function;

#[cfg(test)]
pub(in crate::legalization) fn replay_condition_for_test<'a>(
    function: usize,
    architecture: target::Architecture,
    target: &'a target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    proposed_source: ValueId,
    proposed: &legalized_operations::LegalizedCondition,
) -> Result<conditions::ReplayedCondition<'a>, LegalizationError> {
    conditions::replay(
        function,
        architecture,
        target,
        abstracted,
        optimized,
        proposed_source,
        proposed,
    )
}

/// Independently replay a proposed V9 legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) -> Result<
    (
        usize,
        Option<crate::legalization::model::ProjectedStructuralCallReturnLegalizationReceipt>,
    ),
    LegalizationError,
> {
    validate_replay_custody(target, abstract_plan, unit, proposed)?;
    let projected = projected_structural_call_return::replay(
        target,
        abstract_plan,
        unit,
        &proposed.projected_structural_call_returns,
    )?;

    let decomposition_count =
        ordinary_roster::replay_remaining(target, abstract_plan, unit, proposed)?;
    Ok((decomposition_count, projected))
}
use custody::validate_replay_custody;
