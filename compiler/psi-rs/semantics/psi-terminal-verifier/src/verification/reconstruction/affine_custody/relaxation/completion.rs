//! Independent closed-order completion of one mapped affine bound.

use psi_core::Proposition;

use super::super::super::integer_evidence::closed_integer_less_or_equal;

mod bridge;

pub(super) fn retained(mapped: &Proposition, goal: &Proposition) -> bool {
    bridge::required(mapped, goal)
        .is_some_and(|bridge| closed_integer_less_or_equal(bridge.left, bridge.right))
}
