//! Direct and one-alias landed-literal affine evidence reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody;

mod alias;

pub(super) fn retained_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    if facts()
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
                            root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                        })
                })
                .any(|(root, literal)| {
                    [
                        Proposition::LessOrEqual(literal.clone(), root.clone()),
                        Proposition::LessOrEqual(root.clone(), literal.clone()),
                    ]
                    .iter()
                    .any(|root_bound| {
                        affine_custody::retained_from_root(
                            context,
                            goal,
                            semantic_axioms,
                            root,
                            root_bound,
                        )
                    })
                })
        })
    {
        return true;
    }

    alias::retained(context, goal, requirements, semantic_axioms)
}
