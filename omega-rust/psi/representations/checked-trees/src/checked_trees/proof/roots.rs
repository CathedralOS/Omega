use arena::{Arena, Handle, OwnedIndex};

use symbols::SymbolHandle;

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
    /// Proof-only applications of sealed `FloatSemantics` catalog rows, keyed
    /// by the proof value each produced. Each result rides the same
    /// `float_meaning_projections` table; the projection source stays
    /// transitional until Terminal lowering rejoins it to the
    /// `SemanticApplication` carrier.
    pub float_semantic_applications: Vec<crate::CheckedFloatSemanticApplication>,
    /// Canonical nominal proposition declarations and applications after
    /// transparent aliases and source handles have been eliminated.
    pub proposition_vocabulary: crate::CheckedPropositionVocabulary,
    /// Checked top-level `let`/`boundary let` mathematical declarations — the
    /// PROOF-CONTRACT-MIGRATION surface that replaces proposition
    /// declarations. Each record mirrors one proof-kernel `Declaration`:
    /// universe binders, the parameter telescope, the statement's result, and
    /// a transparent term or named-assumption absence.
    pub mathematical_declarations: Vec<crate::CheckedMathematicalDeclaration>,
    /// The kernel signature `mathematical_declarations` were decided under:
    /// the shared term arena plus the re-decided declaration list in
    /// signature order, kept so lowering can encode the checked signature as
    /// Terminal evidence rather than re-elaborate. `Some` exactly when
    /// `mathematical_declarations` is non-empty — a declaration-bearing
    /// program whose facts were built without it reads as a producer
    /// defect, and lowering admission rejects either half missing.
    pub mathematical_signature: Option<crate::CheckedMathematicalSignature>,
    /// Caller-coordinate -> `contract_calls` row lookup owned by this
    /// ledger; the index carries the `contract_calls` arena's revision, so
    /// a mutation rebuilds it on the next query.
    pub call_index: OwnedIndex<ProofCallIndex>,
}

/// `contract_calls` rows keyed by caller coordinate — `(machine symbol,
/// state symbol, statement index, call ordinal)`. A call's contract row is
/// addressed by its recorded identity rather than position; the last row
/// recorded under a coordinate wins, matching the linear scan the index
/// replaces. The integer hasher suits the handle-keyed map.
#[derive(Clone, Default)]
pub struct ProofCallIndex {
    by_key:
        symbols::SymbolKeyMap<(SymbolHandle, SymbolHandle, usize, usize), Handle<ContractCallFact>>,
}

impl ProofCallIndex {
    /// The row recorded under `key`, if any.
    pub fn get(
        &self,
        key: &(SymbolHandle, SymbolHandle, usize, usize),
    ) -> Option<Handle<ContractCallFact>> {
        self.by_key.get(key).copied()
    }
}

impl ProofFacts {
    /// Run `run` against this ledger's owned caller-coordinate index,
    /// rebuilding when the `contract_calls` arena's revision moved.
    pub fn with_call_index<R>(&self, run: impl FnOnce(&ProofCallIndex) -> R) -> R {
        self.call_index.with(
            self.contract_calls.revision(),
            || {
                let mut index = ProofCallIndex::default();
                for (handle, call) in self.contract_calls.iter() {
                    index.by_key.insert(
                        (
                            call.caller_machine_symbol,
                            call.caller_state_symbol,
                            call.statement_index,
                            call.call_ordinal,
                        ),
                        handle,
                    );
                }
                index
            },
            run,
        )
    }
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
