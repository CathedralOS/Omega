//! Independent direct-root affine/cast/affine replay.

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
    candidates::retained(context, goal, requirements, semantic_axioms, definitions)
}
