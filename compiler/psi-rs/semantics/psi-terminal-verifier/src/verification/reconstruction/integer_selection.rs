//! Independent canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::integer_evidence::{closed_integer_less_or_equal, retained_integer_term_values};
use super::{affine_selection, cast_selection};

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| fact == goal)
    {
        return true;
    }
    match goal {
        Proposition::LessOrEqual(_, _) => {
            requirements
                .iter()
                .chain(semantic_axioms)
                .any(|fact| closed_transitive_integer_bound(goal, fact))
                || retained_literal_integer_bound(goal, requirements, semantic_axioms)
                || retained_two_fact_transitive_integer_bound(goal, requirements, semantic_axioms)
                || retained_equality_substituted_integer_bound(
                    context,
                    goal,
                    requirements,
                    semantic_axioms,
                )
                || context.is_some_and(|context| {
                    cast_selection::retained(context, goal, requirements, semantic_axioms)
                })
                || context.is_some_and(|context| {
                    affine_selection::retained(context, goal, requirements, semantic_axioms)
                })
        }
        Proposition::Conjunction(conjuncts) => {
            !conjuncts.is_empty()
                && conjuncts
                    .iter()
                    .all(|conjunct| retained(context, conjunct, requirements, semantic_axioms))
        }
        Proposition::Disjunction(disjuncts) => disjuncts
            .iter()
            .any(|disjunct| retained(context, disjunct, requirements, semantic_axioms)),
        _ => false,
    }
}

fn retained_equality_substituted_integer_bound(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let facts = || requirements.iter().chain(semantic_axioms);
    if facts().any(|equality| {
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
            requirements
                .iter()
                .chain(semantic_axioms)
                .any(|fact| fact == &relation || closed_transitive_integer_bound(&relation, fact))
                || retained_two_fact_transitive_integer_bound(
                    &relation,
                    requirements,
                    semantic_axioms,
                )
                || context.is_some_and(|context| {
                    affine_selection::retained(context, &relation, requirements, semantic_axioms)
                })
        })
    }) {
        return true;
    }

    let Some(context) = context else {
        return false;
    };
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

fn retained_literal_integer_bound(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(left, right) = goal else {
        return false;
    };
    if let Some((integer_type, left)) = left.integer_value() {
        return retained_integer_term_values(right, requirements, semantic_axioms).any(
            |(known_type, right)| {
                known_type == integer_type
                    && integer_type.admits(right)
                    && integer_type
                        .compare(left, right)
                        .is_some_and(|order| !order.is_gt())
            },
        );
    }
    if let Some((integer_type, right)) = right.integer_value() {
        return retained_integer_term_values(left, requirements, semantic_axioms).any(
            |(known_type, left)| {
                known_type == integer_type
                    && integer_type.admits(left)
                    && integer_type
                        .compare(left, right)
                        .is_some_and(|order| !order.is_gt())
            },
        );
    }
    false
}

fn retained_two_fact_transitive_integer_bound(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    requirements.iter().chain(semantic_axioms).any(|left_fact| {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            return false;
        };
        left == goal_left
            && requirements
                .iter()
                .chain(semantic_axioms)
                .any(|right_fact| {
                    matches!(
                        right_fact,
                        Proposition::LessOrEqual(right_middle, right)
                            if right_middle == middle && right == goal_right
                    )
                })
    })
}

fn closed_transitive_integer_bound(goal: &Proposition, retained: &Proposition) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let Proposition::LessOrEqual(retained_left, retained_right) = retained else {
        return false;
    };
    (retained_right == goal_right && closed_integer_less_or_equal(goal_left, retained_left))
        || (retained_left == goal_left && closed_integer_less_or_equal(retained_right, goal_right))
}
