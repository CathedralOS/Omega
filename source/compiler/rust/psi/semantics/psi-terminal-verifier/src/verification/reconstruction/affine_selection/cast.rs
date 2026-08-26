//! Independent exact cast-adjacent affine precedence.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod direct;
mod endpoint;
mod sandwich;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    direct::retained(context, goal, requirements, semantic_axioms, definitions)
        || sandwich::retained(context, goal, requirements, semantic_axioms, definitions)
}
