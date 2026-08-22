//! Fixed two-citation transitive affine evidence reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody;

mod chains;

use chains::TwoCitationChains;

pub(super) fn retained_transitively_alias_substituted_affine_bound(
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
                    let root_bound = if alias == left {
                        Proposition::LessOrEqual(root.clone(), right.clone())
                    } else if alias == right {
                        Proposition::LessOrEqual(left.clone(), root.clone())
                    } else {
                        return false;
                    };
                    affine_custody::retained_from_root(
                        context,
                        goal,
                        semantic_axioms,
                        root,
                        &root_bound,
                    )
                })
            })
        })
}

pub(super) fn retained_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    TwoCitationChains::new(requirements, semantic_axioms).any(|left_fact, right_fact| {
        let Proposition::LessOrEqual(left, _) = left_fact else {
            unreachable!("only integer chains are enumerated")
        };
        let Proposition::LessOrEqual(_, right) = right_fact else {
            unreachable!("only integer chains are enumerated")
        };
        let root_bound = Proposition::LessOrEqual(left.clone(), right.clone());
        [left, right]
            .into_iter()
            .filter(|root| matches!(root, ScalarTerm::Value { .. }))
            .any(|root| {
                affine_custody::retained_from_root(
                    context,
                    goal,
                    semantic_axioms,
                    root,
                    &root_bound,
                )
            })
    })
}
