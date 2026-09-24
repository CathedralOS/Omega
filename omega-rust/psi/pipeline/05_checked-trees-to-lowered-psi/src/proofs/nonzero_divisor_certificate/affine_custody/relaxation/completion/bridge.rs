//! Producer-local closed-bridge selection for one mapped affine bound.

use semantic_vocabulary::{Proposition, ScalarTerm};

pub(super) enum Position {
    BeforeAffine,
    AfterAffine,
}

pub(super) struct ClosedBridge<'a> {
    pub(super) left: &'a ScalarTerm,
    pub(super) right: &'a ScalarTerm,
    pub(super) position: Position,
}

pub(super) fn required<'a>(
    affine: &'a Proposition,
    goal: &'a Proposition,
) -> Option<ClosedBridge<'a>> {
    let Proposition::LessOrEqual(affine_left, affine_right) = affine else {
        return None;
    };
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    if goal_right == affine_right {
        Some(ClosedBridge {
            left: goal_left,
            right: affine_left,
            position: Position::BeforeAffine,
        })
    } else if goal_left == affine_left {
        Some(ClosedBridge {
            left: affine_right,
            right: goal_right,
            position: Position::AfterAffine,
        })
    } else {
        None
    }
}
