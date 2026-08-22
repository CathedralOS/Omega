//! Independent fixed one- and two-alias affine-root completion replay.

use psi_core::{Proposition, PropositionContext};

use super::super::{affine_custody::DefinitionIndex, alias_transport};

mod completion;

pub(super) fn retained_one(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        completion::retained(
            context,
            goal,
            semantic_axioms,
            definitions,
            root,
            root_bound,
        )
    })
}

pub(super) fn retained_two(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        completion::retained(
            context,
            goal,
            semantic_axioms,
            definitions,
            root,
            root_bound,
        )
    })
}
