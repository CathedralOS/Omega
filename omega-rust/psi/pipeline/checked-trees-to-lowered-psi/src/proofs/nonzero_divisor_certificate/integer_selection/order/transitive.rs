//! Exact two-citation integer transitivity for certificate production.

use std::collections::BTreeMap;

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    // Index the cited `left <= middle` facts once by their left endpoint.
    // The bound producers call this for every endpoint pair they meet, so a
    // per-candidate rescan would square the roster size; the indexed middle
    // join costs one lookup per candidate instead.
    let mut by_left = BTreeMap::<&ScalarTerm, Vec<(Citation, &ScalarTerm, &Proposition)>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, middle) = fact else {
            continue;
        };
        by_left
            .entry(left)
            .or_default()
            .push((citation, middle, fact));
    }
    for &(left_citation, middle, left_fact) in by_left.get(goal_left).into_iter().flatten() {
        for &(right_citation, right, right_fact) in by_left.get(middle).into_iter().flatten() {
            if right == goal_right {
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
