//! Landed-literal, closed-strengthened, and two-citation integer order checks.

use psi_core::Proposition;

use super::super::integer_evidence::retained_integer_term_values;

mod closed;
mod transitive;

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
    transitive::retained(goal, requirements, semantic_axioms)
}

pub(super) fn closed_transitive_integer_bound(goal: &Proposition, retained: &Proposition) -> bool {
    closed::retained(goal, retained)
}
