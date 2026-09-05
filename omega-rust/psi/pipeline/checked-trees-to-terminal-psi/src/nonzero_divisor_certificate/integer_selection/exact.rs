//! Exact retained proposition proof custody.

use proof_admission::ProofNode;
use semantic_vocabulary::Proposition;

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
