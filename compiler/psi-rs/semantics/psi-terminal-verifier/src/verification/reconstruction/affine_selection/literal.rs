//! Direct and one-alias landed-literal affine evidence reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod alias;
mod completion;
mod direct;

pub(super) fn retained_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    if direct::retained(context, goal, requirements, semantic_axioms, definitions) {
        return true;
    }
    alias::retained(context, goal, requirements, semantic_axioms, definitions)
}
