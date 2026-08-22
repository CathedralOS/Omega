//! Independent fixed one-alias cast completion replay.

use psi_core::{Proposition, PropositionContext};

use super::super::super::{alias_transport, cast_custody};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}
