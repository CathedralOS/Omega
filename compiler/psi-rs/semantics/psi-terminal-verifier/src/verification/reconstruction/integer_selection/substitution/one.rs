//! Independent fixed one-equality integer-bound substitution reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::relation;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let facts = || requirements.iter().chain(semantic_axioms);
    facts().any(|equality| {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            return false;
        };
        [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ]
        .into_iter()
        .any(|(old, replacement)| {
            let relation = if old == goal_left {
                Proposition::LessOrEqual(replacement.clone(), goal_right.clone())
            } else if old == goal_right {
                Proposition::LessOrEqual(goal_left.clone(), replacement.clone())
            } else {
                return false;
            };
            relation::retained(context, &relation, requirements, semantic_axioms)
        })
    })
}
