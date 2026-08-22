//! Independent direct landed-literal affine-root reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

mod candidates;

use super::completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    candidates::any(requirements, semantic_axioms, |root, literal| {
        completion::retained(context, goal, semantic_axioms, definitions, root, literal)
    })
}
