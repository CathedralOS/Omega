//! Direct and one-alias landed-literal affine evidence reconstruction.

use psi_core::{Proposition, PropositionContext};

mod alias;
mod direct;

pub(super) fn retained_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if direct::retained(context, goal, requirements, semantic_axioms) {
        return true;
    }
    alias::retained(context, goal, requirements, semantic_axioms)
}
