//! One exact alias substitution around a fixed two-citation affine root bound.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::integer_evidence::cited_facts;
use super::TwoCitationChains;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let chains = TwoCitationChains::new(assumptions, semantic_axioms);

    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if root == alias
                || !matches!(root, psi_core::ScalarTerm::Value { .. })
                || !matches!(alias, psi_core::ScalarTerm::Value { .. })
            {
                continue;
            }
            let proof = chains.find(|left_citation, left_fact, right_citation, right_fact| {
                let Proposition::LessOrEqual(left, _) = left_fact else {
                    unreachable!("only integer chains are enumerated")
                };
                let Proposition::LessOrEqual(_, right) = right_fact else {
                    unreachable!("only integer chains are enumerated")
                };
                completion::prove(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    root,
                    alias,
                    left,
                    right,
                    left_citation.proof(left_fact),
                    right_citation.proof(right_fact),
                    equality_citation.proof(equality),
                )
            });
            if proof.is_some() {
                return proof;
            }
        }
    }
    None
}
