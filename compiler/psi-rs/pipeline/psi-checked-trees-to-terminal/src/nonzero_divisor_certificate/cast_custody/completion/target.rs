//! Producer-local completion of one ordered exact-cast target.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerCastChainWitness, ProofNode, ProofRule, check_certificate};

use super::super::chain;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &ProofNode,
    target: &ScalarTerm,
) -> Option<ProofNode> {
    let definition_axioms = chain::definition_axioms(root, target, semantic_axioms)?;
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerCastBound {
            root_bound: Box::new(root_bound.clone()),
            witness: IntegerCastChainWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
            },
        },
    };
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}
