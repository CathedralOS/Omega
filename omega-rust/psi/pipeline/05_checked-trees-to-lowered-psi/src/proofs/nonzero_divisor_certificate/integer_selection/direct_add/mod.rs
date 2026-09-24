//! Complete direct exact-add selection family.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod conjunction;
mod correlated;
mod flat;
mod relation;
mod targeted;

/// Preserve established direct-add precedence while keeping recursive
/// affine-chain conjunction production as a bounded fallback.
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let relation = relation::classify(goal)?;
    correlated::prove(
        context,
        goal,
        relation.integer_type,
        &relation.left,
        &relation.right,
        &relation.target,
        relation.lower,
        assumptions,
        semantic_axioms,
        definitions,
    )
    .or_else(|| {
        targeted::prove(
            context,
            goal,
            relation.integer_type,
            &relation.left,
            &relation.right,
            &relation.target,
            relation.lower,
            false,
            assumptions,
            semantic_axioms,
            definitions,
        )
    })
    .or_else(|| {
        flat::prove(
            context,
            goal,
            relation.integer_type,
            &relation.left,
            &relation.right,
            &relation.target,
            relation.lower,
            assumptions,
            semantic_axioms,
            definitions,
        )
    })
    .or_else(|| {
        conjunction::prove(
            context,
            goal,
            relation.integer_type,
            &relation.left,
            &relation.right,
            &relation.target,
            relation.lower,
            assumptions,
            semantic_axioms,
            definitions,
        )
    })
    .or_else(|| {
        targeted::prove(
            context,
            goal,
            relation.integer_type,
            &relation.left,
            &relation.right,
            &relation.target,
            relation.lower,
            true,
            assumptions,
            semantic_axioms,
            definitions,
        )
    })
}
