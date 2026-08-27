//! Independent replay of one closed integer-order endpoint bridge.

use psi_core::Proposition;

use super::super::super::integer_evidence::closed_integer_less_or_equal;

pub(super) fn retained(goal: &Proposition, retained: &Proposition) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let Proposition::LessOrEqual(retained_left, retained_right) = retained else {
        return false;
    };
    (retained_right == goal_right && closed_integer_less_or_equal(goal_left, retained_left))
        || (retained_left == goal_left && closed_integer_less_or_equal(retained_right, goal_right))
}
