//! Side-local selection of retained evidence for bounded affine reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::affine_custody::DefinitionIndex;

mod alias;
mod bounds;
mod cast;
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
    let mut definitions = DefinitionIndex::new(semantic_axioms);
    retained_with_definitions(
        context,
        goal,
        requirements,
        semantic_axioms,
        &mut definitions,
    )
}

pub(super) fn retained_with_definitions(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    dispatch::retained(context, goal, requirements, semantic_axioms, definitions)
}
