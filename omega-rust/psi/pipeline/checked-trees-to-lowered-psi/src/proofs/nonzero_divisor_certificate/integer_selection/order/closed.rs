//! One closed endpoint-strengthening bridge for integer order production.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

use super::super::super::integer_evidence::{cited_facts, closed_integer_relation};

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        // A strict fact first weakens to the same non-strict endpoints; the
        // kernel checks both steps, so a cited `v < k` discharges checked
        // evidence such as `v <= maximum - addend` through `k <= bound`.
        let fact_bound = match fact {
            Proposition::LessOrEqual(_, _) => citation.proof(fact),
            Proposition::LessThan(left, right) => ProofNode {
                conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(citation.proof(fact)),
                },
            },
            _ => continue,
        };
        let (Proposition::LessOrEqual(fact_left, fact_right)
        | Proposition::LessThan(fact_left, fact_right)) = fact
        else {
            continue;
        };
        if fact_left == goal_left {
            let tail = Proposition::LessOrEqual(fact_right.clone(), goal_right.clone());
            if let Some(tail) = closed_integer_relation(tail) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(fact_bound),
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
                        middle_less_or_equal_right: Box::new(fact_bound),
                    },
                });
            }
        }
    }
    None
}
