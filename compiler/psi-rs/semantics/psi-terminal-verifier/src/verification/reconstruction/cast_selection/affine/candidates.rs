//! Ordered affine-to-cast endpoint candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(left, right) = goal else {
        return false;
    };
    [(right, left, true), (left, right, false)].into_iter().any(
        |(target, literal, target_is_right)| {
            if !matches!(target, ScalarTerm::Value { .. }) {
                return false;
            }
            completion::retained(
                context,
                goal,
                requirements,
                semantic_axioms,
                target,
                literal,
                target_is_right,
            )
        },
    )
}
