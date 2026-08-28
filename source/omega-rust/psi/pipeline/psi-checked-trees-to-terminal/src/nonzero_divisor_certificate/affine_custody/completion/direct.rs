//! Producer-local direct completion of one affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::{IntegerAffineWitness, ProofNode, ProofRule, check_certificate};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root_bound: &ProofNode,
    witness: &IntegerAffineWitness,
) -> Option<ProofNode> {
    let direct = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness: witness.clone(),
        },
    };
    check_certificate(context, goal, assumptions, semantic_axioms, &direct)
        .is_ok()
        .then_some(direct)
}
