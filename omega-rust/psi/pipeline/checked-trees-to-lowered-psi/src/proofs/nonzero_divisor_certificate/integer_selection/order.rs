//! Exact, closed-strengthened, and two-citation integer order proofs.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

use super::super::integer_evidence::cited_facts;

mod aliases;
mod closed;
mod transitive;

pub(super) fn prove_aliased_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    aliases::prove(goal, assumptions, semantic_axioms)
}

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
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return None;
    }
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    closed::prove(goal, assumptions, semantic_axioms)
}

pub(super) fn prove_equal_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    // Equality (including reflexivity) establishes non-strict integer order.
    // The kernel checks the conversion rather than treating the two
    // propositions as interchangeable citations.
    if let Some(relation) = super::exact::prove(
        &Proposition::Equal(left.clone(), right.clone()),
        assumptions,
        semantic_axioms,
    ) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(relation),
            },
        });
    }
    None
}
