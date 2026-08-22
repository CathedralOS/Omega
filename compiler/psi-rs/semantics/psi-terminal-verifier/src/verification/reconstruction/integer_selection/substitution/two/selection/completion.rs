//! Independent completion of one eligible two-equality endpoint candidate.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::super::affine_selection;

pub(super) fn retained(
    context: &PropositionContext,
    goal_left: &ScalarTerm,
    goal_right: &ScalarTerm,
    target_alias: &ScalarTerm,
    endpoint: usize,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let relation = if endpoint == 0 {
        Proposition::LessOrEqual(target_alias.clone(), goal_right.clone())
    } else {
        Proposition::LessOrEqual(goal_left.clone(), target_alias.clone())
    };
    affine_selection::retained(context, &relation, requirements, semantic_axioms)
}
