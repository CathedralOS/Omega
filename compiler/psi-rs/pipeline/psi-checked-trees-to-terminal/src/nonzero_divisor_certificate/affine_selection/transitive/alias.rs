//! One exact alias substitution around a fixed two-citation affine root bound.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::affine_custody;
use super::super::super::integer_evidence::cited_facts;
use super::TwoCitationChains;

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
                        left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                        middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                    },
                };
                let root_bound = ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(transitive),
                        equality: Box::new(equality_citation.proof(equality)),
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
            });
            if proof.is_some() {
                return proof;
            }
        }
    }
    None
}
