//! Producer-local closed-order completion of one mapped affine bound.

use psi_core::Proposition;
use psi_proof_admission::{ProofNode, ProofRule};

use super::super::super::integer_evidence::closed_integer_relation;

mod bridge;

pub(super) fn prove(goal: &Proposition, affine: ProofNode) -> Option<ProofNode> {
    let bridge = bridge::required(&affine.conclusion, goal)?;
    let bridge_proof = closed_integer_relation(Proposition::LessOrEqual(
        bridge.left.clone(),
        bridge.right.clone(),
    ))?;
    let (left_less_or_equal_middle, middle_less_or_equal_right) = match bridge.position {
        bridge::Position::BeforeAffine => (bridge_proof, affine),
        bridge::Position::AfterAffine => (affine, bridge_proof),
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
            middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
        },
    })
}
