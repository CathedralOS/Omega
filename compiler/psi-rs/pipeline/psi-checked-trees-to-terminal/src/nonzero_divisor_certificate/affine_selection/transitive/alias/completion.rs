//! Fixed transitive-bound alias completion for affine production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::super::affine_custody;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
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
    let root_bound = ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(transitive),
            equality: Box::new(equality),
            endpoint,
        },
    };
    affine_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        root,
        root_bound,
    )
}
