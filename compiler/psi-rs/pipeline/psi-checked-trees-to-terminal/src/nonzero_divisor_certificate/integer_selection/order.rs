//! Exact, closed-strengthened, and two-citation integer order proofs.

use psi_core::Proposition;
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::integer_evidence::{cited_facts, closed_integer_relation};

mod transitive;

pub(super) fn prove_two_fact_transitive_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    transitive::prove(goal, assumptions, semantic_axioms)
}

pub(super) fn prove_exact_or_closed_transitive_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(fact_left, fact_right) = fact else {
            continue;
        };
        if fact_left == goal_left {
            let tail = Proposition::LessOrEqual(fact_right.clone(), goal_right.clone());
            if let Some(tail) = closed_integer_relation(tail) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(citation.proof(fact)),
                        middle_less_or_equal_right: Box::new(tail),
                    },
                });
            }
        }
        if fact_right == goal_right {
            let head = Proposition::LessOrEqual(goal_left.clone(), fact_left.clone());
            if let Some(head) = closed_integer_relation(head) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(head),
                        middle_less_or_equal_right: Box::new(citation.proof(fact)),
                    },
                });
            }
        }
    }
    None
}
