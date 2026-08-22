//! Independent ordered atomic integer-bound reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::{affine_custody::DefinitionIndex, affine_selection, cast_selection};
use super::{order, substitution};

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| order::closed_transitive_integer_bound(goal, fact))
        || order::retained_literal_integer_bound(goal, requirements, semantic_axioms)
        || order::retained_two_fact_transitive_integer_bound(goal, requirements, semantic_axioms)
        || substitution::retained(context, goal, requirements, semantic_axioms, definitions)
        || context.is_some_and(|context| {
            cast_selection::retained(context, goal, requirements, semantic_axioms)
        })
        || context.is_some_and(|context| {
            affine_selection::retained_with_definitions(
                context,
                goal,
                requirements,
                semantic_axioms,
                definitions,
            )
        })
}
