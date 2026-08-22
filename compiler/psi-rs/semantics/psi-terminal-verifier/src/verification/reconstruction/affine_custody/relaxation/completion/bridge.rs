//! Independent closed-bridge selection for one mapped affine bound.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct ClosedBridge<'a> {
    pub(super) left: &'a ScalarTerm,
    pub(super) right: &'a ScalarTerm,
}

pub(super) fn required<'a>(
    mapped: &'a Proposition,
    goal: &'a Proposition,
) -> Option<ClosedBridge<'a>> {
    let Proposition::LessOrEqual(mapped_left, mapped_right) = mapped else {
        return None;
    };
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    if goal_right == mapped_right {
        Some(ClosedBridge {
            left: goal_left,
            right: mapped_left,
        })
    } else if goal_left == mapped_left {
        Some(ClosedBridge {
            left: mapped_right,
            right: goal_right,
        })
    } else {
        None
    }
}
