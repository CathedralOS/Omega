//! Fixed alias-family reconstruction for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};

use super::super::{alias_transport, cast_custody};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    }) {
        return true;
    }
    alias_transport::retained_stronger_cast(context, goal, requirements, semantic_axioms)
        || alias_transport::retained_landed_literal_cast(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || retained_two(context, goal, requirements, semantic_axioms)
}

fn retained_two(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}
