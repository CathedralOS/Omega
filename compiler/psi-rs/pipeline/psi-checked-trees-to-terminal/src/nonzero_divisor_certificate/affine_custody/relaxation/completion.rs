//! Producer-local closed-order completion of one mapped affine bound.

use psi_core::Proposition;
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::integer_evidence::closed_integer_relation;

pub(super) fn prove(goal: &Proposition, affine: ProofNode) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let Proposition::LessOrEqual(affine_left, affine_right) = &affine.conclusion else {
        return None;
    };
    let (left_less_or_equal_middle, middle_less_or_equal_right) = if goal_right == affine_right {
        let bridge = closed_integer_relation(Proposition::LessOrEqual(
            goal_left.clone(),
            affine_left.clone(),
        ))?;
        (bridge, affine)
    } else if goal_left == affine_left {
        let bridge = closed_integer_relation(Proposition::LessOrEqual(
            affine_right.clone(),
            goal_right.clone(),
        ))?;
        (affine, bridge)
    } else {
        return None;
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
            middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
        },
    })
}
