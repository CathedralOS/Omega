//! Side-local selection of retained evidence for bounded affine reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::{affine_custody, alias_transport};

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
        || retained_alias_substituted_affine_bound(context, goal, requirements, semantic_axioms)
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
        || retained_two_alias_substituted_affine_bound(context, goal, requirements, semantic_axioms)
}

fn retained_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        affine_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}

fn retained_two_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        affine_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}
