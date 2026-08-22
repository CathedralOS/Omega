//! Independent direct retained affine-root bound selection.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod candidates;
mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    candidates::any(requirements, semantic_axioms, |root, root_bound| {
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
