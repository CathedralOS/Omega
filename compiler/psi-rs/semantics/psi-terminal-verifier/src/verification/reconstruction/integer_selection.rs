//! Independent canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};

mod bound;
mod logical;
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
            bound::retained(context, goal, requirements, semantic_axioms)
        }
        Proposition::Conjunction(conjuncts) => logical::retained_conjunction(conjuncts, |part| {
            retained(context, part, requirements, semantic_axioms)
        }),
        Proposition::Disjunction(disjuncts) => logical::retained_disjunction(disjuncts, |part| {
            retained(context, part, requirements, semantic_axioms)
        }),
        _ => false,
    }
}
