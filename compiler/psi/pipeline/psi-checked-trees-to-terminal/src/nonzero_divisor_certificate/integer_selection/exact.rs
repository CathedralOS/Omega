//! Exact retained proposition proof custody.

use psi_core::Proposition;
use psi_proof_kernel::ProofNode;

use super::super::integer_evidence::cited_facts;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    cited_facts(assumptions, semantic_axioms)
        .find(|(_, fact)| *fact == goal)
        .map(|(citation, fact)| citation.proof(fact))
}
