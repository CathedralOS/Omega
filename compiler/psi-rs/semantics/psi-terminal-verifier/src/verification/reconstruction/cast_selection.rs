//! Side-local retained-evidence selection for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::{alias_transport, cast_custody};

mod literal;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return false;
    }
    if requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|root_bound| match root_bound {
            Proposition::LessOrEqual(left, right) => Some((root_bound, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            [root_left, root_right]
                .into_iter()
                .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                .any(|root| {
                    cast_custody::retained_from_root(
                        context,
                        goal,
                        semantic_axioms,
                        root,
                        root_bound,
                    )
                })
        })
    {
        return true;
    }
    if literal::retained(context, goal, requirements, semantic_axioms) {
        return true;
    }
    retained_alias_substituted_cast_bound(context, goal, requirements, semantic_axioms)
}

fn retained_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    }) {
        return true;
    }
    alias_transport::retained_stronger_cast(context, goal, requirements, semantic_axioms)
        || alias_transport::retained_landed_literal_cast(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || retained_two_alias_substituted_cast_bound(context, goal, requirements, semantic_axioms)
}

fn retained_two_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}
