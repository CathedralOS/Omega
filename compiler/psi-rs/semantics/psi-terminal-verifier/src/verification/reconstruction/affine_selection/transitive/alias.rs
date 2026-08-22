//! Independent one-alias replay around a fixed two-citation affine root bound.

use psi_core::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

mod candidates;
mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    candidates::AliasedTransitiveCandidates::new(requirements, semantic_axioms).any(
        |root, alias, left, right| {
            completion::retained(
                context,
                goal,
                semantic_axioms,
                definitions,
                root,
                alias,
                left,
                right,
            )
        },
    )
}
