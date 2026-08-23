//! Producer-local direct two-citation transitive root bound.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

pub(super) fn prove(
    left: &ScalarTerm,
    right: &ScalarTerm,
    left_proof: ProofNode,
    right_proof: ProofNode,
) -> ProofNode {
    ProofNode {
        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_proof),
            middle_less_or_equal_right: Box::new(right_proof),
        },
    }
}
