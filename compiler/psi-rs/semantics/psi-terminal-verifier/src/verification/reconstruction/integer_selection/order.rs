//! Landed-literal, closed-strengthened, and two-citation integer order checks.

use psi_core::Proposition;

use super::super::integer_evidence::{closed_integer_less_or_equal, retained_integer_term_values};

pub(super) fn retained_literal_integer_bound(
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

pub(super) fn retained_two_fact_transitive_integer_bound(
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

pub(super) fn closed_transitive_integer_bound(goal: &Proposition, retained: &Proposition) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let Proposition::LessOrEqual(retained_left, retained_right) = retained else {
        return false;
    };
    (retained_right == goal_right && closed_integer_less_or_equal(goal_left, retained_left))
        || (retained_left == goal_left && closed_integer_less_or_equal(retained_right, goal_right))
}
