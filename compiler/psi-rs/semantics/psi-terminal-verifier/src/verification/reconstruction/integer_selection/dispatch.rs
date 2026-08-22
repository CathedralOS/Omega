//! Independent canonical integer proposition-kind reconstruction dispatch.

use psi_core::{Proposition, PropositionContext};

use super::{bound, logical};

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut retained_part: impl FnMut(&Proposition) -> bool,
) -> bool {
    match goal {
        Proposition::LessOrEqual(_, _) => {
            bound::retained(context, goal, requirements, semantic_axioms)
        }
        Proposition::Conjunction(conjuncts) => {
            logical::retained_conjunction(conjuncts, &mut retained_part)
        }
        Proposition::Disjunction(disjuncts) => {
            logical::retained_disjunction(disjuncts, &mut retained_part)
        }
        _ => false,
    }
}
