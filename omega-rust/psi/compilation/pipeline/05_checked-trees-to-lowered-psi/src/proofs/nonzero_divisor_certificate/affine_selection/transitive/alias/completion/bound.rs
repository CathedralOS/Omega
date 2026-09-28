//! Producer-local alias-substituted transitive affine root bound.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    left: &ScalarTerm,
    right: &ScalarTerm,
    left_proof: ProofNode,
    right_proof: ProofNode,
    equality: ProofNode,
) -> Option<ProofNode> {
    let (endpoint, conclusion) = if alias == left {
        (0, Proposition::LessOrEqual(root.clone(), right.clone()))
    } else if alias == right {
        (1, Proposition::LessOrEqual(left.clone(), root.clone()))
    } else {
        return None;
    };
    let transitive = ProofNode {
        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_proof),
            middle_less_or_equal_right: Box::new(right_proof),
        },
    };
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(transitive),
            equality: Box::new(equality),
            endpoint,
        },
    })
}
