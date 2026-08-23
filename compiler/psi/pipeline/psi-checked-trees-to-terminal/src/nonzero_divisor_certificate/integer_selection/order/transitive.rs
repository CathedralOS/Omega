//! Exact two-citation integer transitivity for certificate production.

use psi_core::Proposition;
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::integer_evidence::cited_facts;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (left_citation, left_fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            continue;
        };
        if left != goal_left {
            continue;
        }
        for (right_citation, right_fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(right_middle, right) = right_fact else {
                continue;
            };
            if right_middle == middle && right == goal_right {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                        middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                    },
                });
            }
        }
    }
    None
}
