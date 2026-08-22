//! Independent one-alias replay around a fixed two-citation affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::TwoCitationChains;

mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    let chains = TwoCitationChains::new(requirements, semantic_axioms);

    facts()
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(equality_left, equality_right)| {
            [
                (equality_left, equality_right),
                (equality_right, equality_left),
            ]
            .into_iter()
            .filter(|(root, alias)| {
                root != alias
                    && matches!(root, ScalarTerm::Value { .. })
                    && matches!(alias, ScalarTerm::Value { .. })
            })
            .any(|(root, alias)| {
                chains.any(|left_fact, right_fact| {
                    let Proposition::LessOrEqual(left, _) = left_fact else {
                        unreachable!("only integer chains are enumerated")
                    };
                    let Proposition::LessOrEqual(_, right) = right_fact else {
                        unreachable!("only integer chains are enumerated")
                    };
                    completion::retained(context, goal, semantic_axioms, root, alias, left, right)
                })
            })
        })
}
