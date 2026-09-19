//! Optimizer module role: executable entrance. Independent replay of a proposed legal-operation projection.

mod custody;
mod ordinary_roster;
mod scalar_graph;
mod shared;

use shared::*;

/// Independently replay a proposed legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
    verified_input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<(), LegalizationError> {
    validate_replay_custody(target, abstract_plan, unit, proposed, verified_input)?;
    // A retained callback argument must join to one exact normalized foreign
    // row before any legalized instruction replays against this plan.
    super::scalar_graph_input::normalized_foreign::validate_native_callback_roster(target)?;

    ordinary_roster::replay_remaining(target, abstract_plan, unit, proposed)
}
use custody::validate_replay_custody;
