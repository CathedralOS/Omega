//! Side-local selection of retained evidence for bounded affine reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::affine_custody::DefinitionIndex;

mod alias;
mod bounds;
mod direct;
mod dispatch;
mod equalities;
mod fact_identity;
mod literal;
mod transitive;
mod value_index;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let definitions = DefinitionIndex::new(semantic_axioms);
    dispatch::retained(context, goal, requirements, semantic_axioms, &definitions)
}
