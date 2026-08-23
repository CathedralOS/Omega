//! Source-ordered landed-literal cast candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(left, right)| {
            [(left, right), (right, left)]
                .into_iter()
                .filter(|(root, literal)| {
                    matches!(root, ScalarTerm::Value { .. })
                        && literal.integer_value().is_some_and(|(integer_type, _)| {
                            root.scalar_type() == ScalarType::Integer(integer_type)
                        })
                })
                .any(|(root, literal)| {
                    completion::retained(context, goal, semantic_axioms, root, literal)
                })
        })
}
