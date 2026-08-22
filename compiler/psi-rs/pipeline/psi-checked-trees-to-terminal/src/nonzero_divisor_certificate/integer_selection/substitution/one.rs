//! Fixed one-equality integer-bound substitution production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::integer_evidence::cited_facts;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = fact else {
            continue;
        };
        for (old, replacement) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if let Some(proof) = completion::prove(
                context,
                goal,
                goal_left,
                goal_right,
                old,
                replacement,
                assumptions,
                semantic_axioms,
                citation.proof(fact),
            ) {
                return Some(proof);
            }
        }
    }

    None
}
