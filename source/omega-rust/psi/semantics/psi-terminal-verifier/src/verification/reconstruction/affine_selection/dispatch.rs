//! Exact affine evidence precedence for independent reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;
use super::{alias, cast, direct, literal, transitive};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    direct::retained(context, goal, requirements, semantic_axioms, definitions)
        || literal::retained_landed_literal_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
            definitions,
        )
        || alias::retained_one(context, goal, requirements, semantic_axioms, definitions)
        || transitive::retained_transitively_reconstructed_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
            definitions,
        )
        || transitive::retained_transitively_alias_substituted_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
            definitions,
        )
        || alias::retained_two(context, goal, requirements, semantic_axioms, definitions)
        || cast::retained(context, goal, requirements, semantic_axioms, definitions)
}
