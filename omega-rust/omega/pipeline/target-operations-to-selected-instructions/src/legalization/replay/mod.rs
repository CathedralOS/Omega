//! Optimizer module role: executable entrance. Independent replay of a proposed legal-operation projection.

mod custody;
mod ordinary_roster;
mod scalar_graph;
mod shared;
mod structural;
mod validators;

use crate::legalization::projected_structural_call_return;
use shared::*;
use structural::replay_structural_unit_function;

/// Independently replay a proposed legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<
    (
        usize,
        Option<crate::legalization::model::ProjectedStructuralCallReturnLegalizationReceipt>,
    ),
    LegalizationError,
> {
    validate_replay_custody(target, abstract_plan, unit, proposed, verified_input)?;
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
