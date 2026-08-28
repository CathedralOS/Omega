//! Independent canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};

use super::affine_custody::DefinitionIndex;

mod bound;
mod dispatch;
mod exact;
mod logical;
mod order;
mod substitution;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    if exact::retained(goal, requirements, semantic_axioms) {
        return true;
    }
    if let Some(retained) =
        dispatch::retained_atomic(context, goal, requirements, semantic_axioms, definitions)
    {
        return retained;
    }
    match goal {
        Proposition::Conjunction(conjuncts) => logical::retained_conjunction(conjuncts, |part| {
            retained(context, part, requirements, semantic_axioms, definitions)
        }),
        Proposition::Disjunction(disjuncts) => logical::retained_disjunction(disjuncts, |part| {
            retained(context, part, requirements, semantic_axioms, definitions)
        }),
        _ => false,
    }
}
