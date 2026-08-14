use psi_arena::Arena;

use crate::{
    CheckedEvidenceTerm, ContractCallFact, ContractExitFact, ContractOperatorUseFact,
    ContractProofFact, ContractProofFactRef, ProofObligationFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
    pub contract_facts: Arena<ContractProofFact>,
    pub evidence_terms: Arena<CheckedEvidenceTerm>,
    pub contract_fact_refs: Arena<ContractProofFactRef>,
    pub contract_calls: Arena<ContractCallFact>,
    pub contract_exits: Arena<ContractExitFact>,
    pub contract_operator_uses: Arena<ContractOperatorUseFact>,
    /// Canonical nominal proposition declarations and applications after
    /// transparent aliases and source handles have been eliminated.
    pub proposition_vocabulary: crate::CheckedPropositionVocabulary,
}

impl ProofFacts {
    pub fn with_roots(
        obligations: Arena<ProofObligationFact>,
        contract_facts: Arena<ContractProofFact>,
        evidence_terms: Arena<CheckedEvidenceTerm>,
        contract_fact_refs: Arena<ContractProofFactRef>,
        contract_calls: Arena<ContractCallFact>,
        contract_exits: Arena<ContractExitFact>,
        contract_operator_uses: Arena<ContractOperatorUseFact>,
        proposition_vocabulary: crate::CheckedPropositionVocabulary,
    ) -> Self {
        Self {
            obligations,
            contract_facts,
            evidence_terms,
            contract_fact_refs,
            contract_calls,
            contract_exits,
            contract_operator_uses,
            proposition_vocabulary,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CheckedEvidenceTerm, ContractCallFact, ContractExitFact, ContractOperatorUseFact,
        ContractProofFact, ContractProofFactRef, ProofFacts, ProofObligationFact,
    };
    use psi_arena::Arena;

    #[test]
    fn proof_facts_constructor_keeps_proof_roots_explicit() {
        let obligations = Arena::<ProofObligationFact>::with_capacity(1);
        let contract_facts = Arena::<ContractProofFact>::with_capacity(2);
        let evidence_terms = Arena::<CheckedEvidenceTerm>::with_capacity(2);
        let contract_fact_refs = Arena::<ContractProofFactRef>::with_capacity(3);
        let contract_calls = Arena::<ContractCallFact>::with_capacity(4);
        let contract_exits = Arena::<ContractExitFact>::with_capacity(5);
        let contract_operator_uses = Arena::<ContractOperatorUseFact>::with_capacity(6);
        let proposition_vocabulary = crate::CheckedPropositionVocabulary::default();

        let facts = ProofFacts::with_roots(
            obligations.clone(),
            contract_facts.clone(),
            evidence_terms.clone(),
            contract_fact_refs.clone(),
            contract_calls.clone(),
            contract_exits.clone(),
            contract_operator_uses.clone(),
            proposition_vocabulary.clone(),
        );

        assert_eq!(facts.obligations, obligations);
        assert_eq!(facts.contract_facts, contract_facts);
        assert_eq!(facts.evidence_terms, evidence_terms);
        assert_eq!(facts.contract_fact_refs, contract_fact_refs);
        assert_eq!(facts.contract_calls, contract_calls);
        assert_eq!(facts.contract_exits, contract_exits);
        assert_eq!(facts.contract_operator_uses, contract_operator_uses);
        assert_eq!(facts.proposition_vocabulary, proposition_vocabulary);
    }
}
