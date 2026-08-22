//! Independent canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};

use super::integer_evidence::{closed_integer_less_or_equal, retained_integer_term_values};
use super::{affine_selection, cast_selection};

mod substitution;

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
                || substitution::retained(context, goal, requirements, semantic_axioms)
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
