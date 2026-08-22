//! Side-local selection of retained evidence for bounded affine reconstruction.

use psi_core::{Proposition, PropositionContext};

mod alias;
mod direct;
mod literal;
mod transitive;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if direct::retained(context, goal, requirements, semantic_axioms) {
        return true;
    }
    literal::retained_landed_literal_affine_bound(context, goal, requirements, semantic_axioms)
        || alias::retained_one(context, goal, requirements, semantic_axioms)
        || transitive::retained_transitively_reconstructed_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || transitive::retained_transitively_alias_substituted_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || alias::retained_two(context, goal, requirements, semantic_axioms)
}
