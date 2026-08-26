use psi_arena::Arena;

use crate::{
    CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument, ContractExitFact,
    ContractOperatorUseFact, ContractProofFact, ContractProofFactRef, EvidenceForwardingFact,
    OutcomeSpecificArmFact, OutcomeSpecificGuaranteeFact, ProofObligationFact, ProofOutputCallFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
    pub contract_facts: Arena<ContractProofFact>,
    pub outcome_specific_guarantees: Arena<OutcomeSpecificGuaranteeFact>,
    pub outcome_specific_arms: Arena<OutcomeSpecificArmFact>,
    pub evidence_terms: Arena<CheckedEvidenceTerm>,
    pub contract_evidence_arguments: Arena<ContractEvidenceArgument>,
    pub evidence_forwardings: Arena<EvidenceForwardingFact>,
    pub proof_output_calls: Arena<ProofOutputCallFact>,
    pub contract_fact_refs: Arena<ContractProofFactRef>,
    pub contract_calls: Arena<ContractCallFact>,
    pub contract_exits: Arena<ContractExitFact>,
    pub contract_operator_uses: Arena<ContractOperatorUseFact>,
    /// Proof-only float projections bound to actual validated source calls.
    /// Rows retain only plan-local value identities and exact landed formats.
    pub float_meaning_projections: Vec<crate::CheckedFloatMeaningProjection>,
    /// Proof-position equalities over exact float-meaning projection results.
    pub float_meaning_equalities: Vec<crate::CheckedFloatMeaningEqualityProposition>,
    /// Canonical nominal proposition declarations and applications after
    /// transparent aliases and source handles have been eliminated.
    pub proposition_vocabulary: crate::CheckedPropositionVocabulary,
}

impl ProofFacts {
    pub fn with_roots(
        obligations: Arena<ProofObligationFact>,
        contract_facts: Arena<ContractProofFact>,
        outcome_specific_guarantees: Arena<OutcomeSpecificGuaranteeFact>,
        outcome_specific_arms: Arena<OutcomeSpecificArmFact>,
        evidence_terms: Arena<CheckedEvidenceTerm>,
        contract_evidence_arguments: Arena<ContractEvidenceArgument>,
        evidence_forwardings: Arena<EvidenceForwardingFact>,
        proof_output_calls: Arena<ProofOutputCallFact>,
        contract_fact_refs: Arena<ContractProofFactRef>,
        contract_calls: Arena<ContractCallFact>,
        contract_exits: Arena<ContractExitFact>,
        contract_operator_uses: Arena<ContractOperatorUseFact>,
        float_meaning_projections: Vec<crate::CheckedFloatMeaningProjection>,
        float_meaning_equalities: Vec<crate::CheckedFloatMeaningEqualityProposition>,
        proposition_vocabulary: crate::CheckedPropositionVocabulary,
    ) -> Self {
        Self {
            obligations,
            contract_facts,
            outcome_specific_guarantees,
            outcome_specific_arms,
            evidence_terms,
            contract_evidence_arguments,
            evidence_forwardings,
            proof_output_calls,
            contract_fact_refs,
            contract_calls,
            contract_exits,
            contract_operator_uses,
            float_meaning_projections,
            float_meaning_equalities,
            proposition_vocabulary,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument, ContractExitFact,
        ContractOperatorUseFact, ContractProofFact, ContractProofFactRef, EvidenceForwardingFact,
        OutcomeSpecificArmFact, OutcomeSpecificGuaranteeFact, ProofFacts, ProofObligationFact,
        ProofOutputCallFact,
    };
    use psi_arena::Arena;

    #[test]
    fn proof_facts_constructor_keeps_proof_roots_explicit() {
        let obligations = Arena::<ProofObligationFact>::with_capacity(1);
        let contract_facts = Arena::<ContractProofFact>::with_capacity(2);
        let outcome_specific_guarantees = Arena::<OutcomeSpecificGuaranteeFact>::with_capacity(2);
        let outcome_specific_arms = Arena::<OutcomeSpecificArmFact>::with_capacity(2);
        let evidence_terms = Arena::<CheckedEvidenceTerm>::with_capacity(2);
        let contract_evidence_arguments = Arena::<ContractEvidenceArgument>::with_capacity(2);
        let evidence_forwardings = Arena::<EvidenceForwardingFact>::with_capacity(2);
        let proof_output_calls = Arena::<ProofOutputCallFact>::with_capacity(2);
        let contract_fact_refs = Arena::<ContractProofFactRef>::with_capacity(3);
        let contract_calls = Arena::<ContractCallFact>::with_capacity(4);
        let contract_exits = Arena::<ContractExitFact>::with_capacity(5);
        let contract_operator_uses = Arena::<ContractOperatorUseFact>::with_capacity(6);
        let float_meaning_projections = Vec::new();
        let float_meaning_equalities = Vec::new();
        let proposition_vocabulary = crate::CheckedPropositionVocabulary::default();

        let facts = ProofFacts::with_roots(
            obligations.clone(),
            contract_facts.clone(),
            outcome_specific_guarantees.clone(),
            outcome_specific_arms.clone(),
            evidence_terms.clone(),
            contract_evidence_arguments.clone(),
            evidence_forwardings.clone(),
            proof_output_calls.clone(),
            contract_fact_refs.clone(),
            contract_calls.clone(),
            contract_exits.clone(),
            contract_operator_uses.clone(),
            float_meaning_projections.clone(),
            float_meaning_equalities.clone(),
            proposition_vocabulary.clone(),
        );

        assert_eq!(facts.obligations, obligations);
        assert_eq!(facts.contract_facts, contract_facts);
        assert_eq!(
            facts.outcome_specific_guarantees,
            outcome_specific_guarantees
        );
        assert_eq!(facts.outcome_specific_arms, outcome_specific_arms);
        assert_eq!(facts.evidence_terms, evidence_terms);
        assert_eq!(
            facts.contract_evidence_arguments,
            contract_evidence_arguments
        );
        assert_eq!(facts.evidence_forwardings, evidence_forwardings);
        assert_eq!(facts.proof_output_calls, proof_output_calls);
        assert_eq!(facts.contract_fact_refs, contract_fact_refs);
        assert_eq!(facts.contract_calls, contract_calls);
        assert_eq!(facts.contract_exits, contract_exits);
        assert_eq!(facts.contract_operator_uses, contract_operator_uses);
        assert_eq!(facts.float_meaning_projections, float_meaning_projections);
        assert_eq!(facts.float_meaning_equalities, float_meaning_equalities);
        assert_eq!(facts.proposition_vocabulary, proposition_vocabulary);
    }
}
