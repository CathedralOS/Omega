use arena::Arena;

use crate::{
    CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument, ContractExitFact,
    ContractExpressionEvidenceCallFact, ContractExpressionStaticConformanceApplicationFact,
    ContractOperatorUseFact, ContractProofFact, ContractProofFactRef, EvidenceForwardingFact,
    InheritedContractScope, OutcomeSpecificArmFact, OutcomeSpecificGuaranteeFact,
    ProofObligationFact, ProofOutputCallFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    /// Checked-IR certificates for exact authored ensures obligations that the
    /// structural validator stood down from but the proof kernel discharged
    /// by an exact machine-requires assumption.
    pub contract_entailment_assumption_discharges:
        Vec<crate::CheckedContractEntailmentAssumptionDischarge>,
    pub obligations: Arena<ProofObligationFact>,
    pub contract_facts: Arena<ContractProofFact>,
    /// Conformance-edge provenance for contract facts inherited from trait
    /// requirements. Rows are referenced from `ContractProofFact::inherited_scope`
    /// and carry the exact trait arguments used to instantiate the schema.
    pub inherited_contract_scopes: Arena<InheritedContractScope>,
    pub outcome_specific_guarantees: Arena<OutcomeSpecificGuaranteeFact>,
    pub outcome_specific_arms: Arena<OutcomeSpecificArmFact>,
    pub evidence_terms: Arena<CheckedEvidenceTerm>,
    pub contract_evidence_arguments: Arena<ContractEvidenceArgument>,
    pub evidence_forwardings: Arena<EvidenceForwardingFact>,
    pub proof_output_calls: Arena<ProofOutputCallFact>,
    pub contract_fact_refs: Arena<ContractProofFactRef>,
    pub contract_calls: Arena<ContractCallFact>,
    pub contract_expression_evidence_calls: Vec<ContractExpressionEvidenceCallFact>,
    pub contract_expression_static_conformance_applications:
        Vec<ContractExpressionStaticConformanceApplicationFact>,
    pub contract_exits: Arena<ContractExitFact>,
    pub contract_operator_uses: Arena<ContractOperatorUseFact>,
    /// Unique proof-only float projections bound to validated source keys.
    /// Rows retain only plan-local value identities and exact landed formats.
    pub float_meaning_projections: Vec<crate::CheckedFloatMeaningProjection>,
    /// Authored projection occurrences and spans, retained separately from
    /// canonical proof-value identity and erased before Terminal Psi.
    pub float_meaning_projection_occurrences: Vec<crate::CheckedFloatMeaningProjectionOccurrence>,
    /// Proof-position equalities over exact float-meaning projection results.
    pub float_meaning_equalities: Vec<crate::CheckedFloatMeaningEqualityProposition>,
    /// Canonical nominal proposition declarations and applications after
    /// transparent aliases and source handles have been eliminated.
    pub proposition_vocabulary: crate::CheckedPropositionVocabulary,
    /// Checked top-level `let`/`boundary let` mathematical declarations — the
    /// PROOF-CONTRACT-MIGRATION surface that replaces proposition
    /// declarations. Each record mirrors one proof-kernel `Declaration`:
    /// universe binders, the parameter telescope, the statement's result, and
    /// a transparent term or named-assumption absence.
    pub mathematical_declarations: Vec<crate::CheckedMathematicalDeclaration>,
}

impl ProofFacts {
    /// Rejoin one exact authored equality to the unique checked direct-result
    /// FloatMeaning projection it reflexively names. Consumers still own their
    /// stage-specific result-shape and contract-lane checks. Transported
    /// `ensures` instances share the declaration's `source_expression`, so the
    /// rejoin keys (owner, expression, use site) and only declaration rows —
    /// those with no use-site coordinate — can discharge an authored exit.
    pub fn direct_result_float_meaning_reflexivity(
        &self,
        owner_machine: symbols::SymbolHandle,
        source_expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&crate::CheckedFloatMeaningProjection> {
        let mut equalities = self.float_meaning_equalities.iter().filter(|equality| {
            equality.source_expression == source_expression && equality.use_site.is_none()
        });
        let equality = equalities.next()?;
        if equalities.next().is_some() || equality.left != equality.right {
            return None;
        }
        let mut projections = self
            .float_meaning_projections
            .iter()
            .filter(|projection| projection.result.id == equality.left);
        let projection = projections.next()?;
        if projections.next().is_some()
            || !matches!(
                projection.source,
                crate::CheckedFloatProjectionSource::DirectMachineResult(result)
                    if result.owner_machine == owner_machine
            )
        {
            return None;
        }
        Some(projection)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument, ContractExitFact,
        ContractOperatorUseFact, ContractProofFact, ContractProofFactRef, EvidenceForwardingFact,
        InheritedContractScope, OutcomeSpecificArmFact, OutcomeSpecificGuaranteeFact, ProofFacts,
        ProofObligationFact, ProofOutputCallFact,
    };
    use arena::Arena;

    #[test]
    fn proof_facts_constructor_keeps_proof_roots_explicit() {
        let obligations = Arena::<ProofObligationFact>::with_capacity(1);
        let contract_facts = Arena::<ContractProofFact>::with_capacity(2);
        let inherited_contract_scopes = Arena::<InheritedContractScope>::with_capacity(2);
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

        let facts = ProofFacts {
            obligations: obligations.clone(),
            contract_facts: contract_facts.clone(),
            inherited_contract_scopes: inherited_contract_scopes.clone(),
            outcome_specific_guarantees: outcome_specific_guarantees.clone(),
            outcome_specific_arms: outcome_specific_arms.clone(),
            evidence_terms: evidence_terms.clone(),
            contract_evidence_arguments: contract_evidence_arguments.clone(),
            evidence_forwardings: evidence_forwardings.clone(),
            proof_output_calls: proof_output_calls.clone(),
            contract_fact_refs: contract_fact_refs.clone(),
            contract_calls: contract_calls.clone(),
            contract_exits: contract_exits.clone(),
            contract_operator_uses: contract_operator_uses.clone(),
            float_meaning_projections: float_meaning_projections.clone(),
            float_meaning_equalities: float_meaning_equalities.clone(),
            proposition_vocabulary: proposition_vocabulary.clone(),
            ..ProofFacts::default()
        };

        assert_eq!(facts.obligations, obligations);
        assert!(facts.contract_entailment_assumption_discharges.is_empty());
        assert_eq!(facts.contract_facts, contract_facts);
        assert_eq!(facts.inherited_contract_scopes, inherited_contract_scopes);
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
        assert!(facts.contract_expression_evidence_calls.is_empty());
        assert!(
            facts
                .contract_expression_static_conformance_applications
                .is_empty()
        );
        assert_eq!(facts.contract_exits, contract_exits);
        assert_eq!(facts.contract_operator_uses, contract_operator_uses);
        assert_eq!(facts.float_meaning_projections, float_meaning_projections);
        assert_eq!(facts.float_meaning_equalities, float_meaning_equalities);
        assert_eq!(facts.proposition_vocabulary, proposition_vocabulary);
        assert!(facts.mathematical_declarations.is_empty());
    }
}
