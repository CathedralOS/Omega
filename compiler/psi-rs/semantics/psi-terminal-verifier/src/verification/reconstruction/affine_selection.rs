//! Side-local selection of retained evidence for bounded affine reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::affine_custody::DefinitionIndex;

mod alias;
mod bounds;
mod direct;
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
    if direct::retained(context, goal, requirements, semantic_axioms, &definitions) {
        return true;
    }
    literal::retained_landed_literal_affine_bound(
        context,
        goal,
        requirements,
        semantic_axioms,
        &definitions,
    ) || alias::retained_one(context, goal, requirements, semantic_axioms, &definitions)
        || transitive::retained_transitively_reconstructed_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
            &definitions,
        )
        || transitive::retained_transitively_alias_substituted_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
            &definitions,
        )
        || alias::retained_two(context, goal, requirements, semantic_axioms, &definitions)
}
