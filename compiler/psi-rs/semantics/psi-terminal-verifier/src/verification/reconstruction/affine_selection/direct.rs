//! Independent direct retained affine-root bound selection.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::{self, DefinitionIndex};

mod candidates;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    candidates::any(requirements, semantic_axioms, |root, root_bound| {
        affine_custody::retained_from_root(
            context,
            goal,
            semantic_axioms,
            definitions,
            root,
            root_bound,
        )
    })
}
