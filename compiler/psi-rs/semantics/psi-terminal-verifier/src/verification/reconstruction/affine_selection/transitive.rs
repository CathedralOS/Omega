//! Fixed two-citation transitive affine evidence reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod alias;
mod chains;
mod completion;

use chains::TwoCitationChains;

pub(super) fn retained_transitively_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    alias::retained(context, goal, requirements, semantic_axioms, definitions)
}

pub(super) fn retained_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    TwoCitationChains::new(requirements, semantic_axioms).any(|left, right| {
        completion::retained(context, goal, semantic_axioms, definitions, left, right)
    })
}
