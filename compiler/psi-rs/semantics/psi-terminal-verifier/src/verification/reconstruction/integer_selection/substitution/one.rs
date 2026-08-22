//! Independent fixed one-equality integer-bound substitution reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

mod completion;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
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
            completion::retained(
                context,
                goal_left,
                goal_right,
                old,
                replacement,
                requirements,
                semantic_axioms,
                definitions,
            )
        })
    })
}
