//! Exact, closed-strengthened, and two-citation integer order proofs.

use psi_core::Proposition;
use psi_proof_admission::ProofNode;

use super::super::integer_evidence::cited_facts;

mod closed;
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
