//! Independent canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};

use super::{affine_selection, cast_selection};

mod order;
mod substitution;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| fact == goal)
    {
        return true;
    }
    match goal {
        Proposition::LessOrEqual(_, _) => {
            requirements
                .iter()
                .chain(semantic_axioms)
                .any(|fact| order::closed_transitive_integer_bound(goal, fact))
                || order::retained_literal_integer_bound(goal, requirements, semantic_axioms)
                || order::retained_two_fact_transitive_integer_bound(
                    goal,
                    requirements,
                    semantic_axioms,
                )
                || substitution::retained(context, goal, requirements, semantic_axioms)
                || context.is_some_and(|context| {
                    cast_selection::retained(context, goal, requirements, semantic_axioms)
                })
                || context.is_some_and(|context| {
                    affine_selection::retained(context, goal, requirements, semantic_axioms)
                })
        }
        Proposition::Conjunction(conjuncts) => {
            !conjuncts.is_empty()
                && conjuncts
                    .iter()
                    .all(|conjunct| retained(context, conjunct, requirements, semantic_axioms))
        }
        Proposition::Disjunction(disjuncts) => disjuncts
            .iter()
            .any(|disjunct| retained(context, disjunct, requirements, semantic_axioms)),
        _ => false,
    }
}
