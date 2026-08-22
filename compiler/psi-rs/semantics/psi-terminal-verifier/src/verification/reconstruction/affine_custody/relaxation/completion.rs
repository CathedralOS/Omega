//! Independent closed-order completion of one mapped affine bound.

use psi_core::Proposition;

use super::super::super::integer_evidence::closed_integer_less_or_equal;

pub(super) fn retained(mapped: &Proposition, goal: &Proposition) -> bool {
    let Proposition::LessOrEqual(mapped_left, mapped_right) = mapped else {
        return false;
    };
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    (goal_right == mapped_right && closed_integer_less_or_equal(goal_left, mapped_left))
        || (goal_left == mapped_left && closed_integer_less_or_equal(mapped_right, goal_right))
}
