//! Verifier-local fixed two-equality affine endpoint selection.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_selection;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let facts = || requirements.iter().chain(semantic_axioms);
    facts()
        .filter_map(|outer_equality| match outer_equality {
            Proposition::Equal(left, right) => Some((outer_equality, left, right)),
            _ => None,
        })
        .any(|(outer_equality, outer_left, outer_right)| {
            [(outer_left, outer_right), (outer_right, outer_left)]
                .into_iter()
                .filter_map(|(old, middle_alias)| {
                    let endpoint = if old == goal_left {
                        0
                    } else if old == goal_right {
                        1
                    } else {
                        return None;
                    };
                    (matches!(old, ScalarTerm::Value { .. })
                        && matches!(middle_alias, ScalarTerm::Value { .. })
                        && old != middle_alias
                        && old.scalar_type() == middle_alias.scalar_type())
                    .then_some((old, middle_alias, endpoint))
                })
                .any(|(old, middle_alias, endpoint)| {
                    facts()
                        .filter(|inner_equality| !std::ptr::eq(outer_equality, *inner_equality))
                        .filter_map(|inner_equality| match inner_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(inner_left, inner_right)| {
                            let target_alias = if inner_left == middle_alias {
                                inner_right
                            } else if inner_right == middle_alias {
                                inner_left
                            } else {
                                return false;
                            };
                            if !matches!(target_alias, ScalarTerm::Value { .. })
                                || target_alias == old
                                || target_alias == middle_alias
                                || target_alias.scalar_type() != old.scalar_type()
                            {
                                return false;
                            }
                            let relation = if endpoint == 0 {
                                Proposition::LessOrEqual(target_alias.clone(), goal_right.clone())
                            } else {
                                Proposition::LessOrEqual(goal_left.clone(), target_alias.clone())
                            };
                            affine_selection::retained(
                                context,
                                &relation,
                                requirements,
                                semantic_axioms,
                            )
                        })
                })
        })
}
