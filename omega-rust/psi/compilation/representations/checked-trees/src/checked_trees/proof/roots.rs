use arena::{Arena, Handle};

use symbols::SymbolHandle;

use crate::checked_trees::keyed_rows::KeyedRows;
use crate::{
    CallContractSignature, CheckedEvidenceTerm, ContractCallFact, ContractEvidenceArgument,
    ContractExitFact, ContractExpressionEvidenceCallFact,
    ContractExpressionStaticConformanceApplicationFact, ContractOperatorUseFact, ContractProofFact,
    ContractProofFactRef, EvidenceForwardingFact, InheritedContractScope, OutcomeSpecificArmFact,
    OutcomeSpecificGuaranteeFact, ProofObligationFact, ProofOutputCallFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
    pub integer_assignments: Arena<crate::CheckedIntegerAssignmentDecision>,
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
    pub call_contract_signatures: Arena<CallContractSignature>,
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
    /// Independently retained source carrier formation, sharing that signature.
    pub source_evidence_carriers: Vec<crate::CheckedSourceEvidenceCarrier>,
    /// Independently audited integer-theory grants in that exact signature.
    pub mathematical_safety_assumptions: Vec<u32>,
    pub indexed_theorems: Vec<crate::CheckedIndexedTheorem>,
    pub indexed_citations: Vec<crate::CheckedIndexedCitation>,
    /// Caller-coordinate -> `contract_calls` row lookup. The grouping is
    /// rebuilt whenever `contract_calls` moves; a call's contract row is
    /// addressed by its recorded identity rather than its position.
    pub call_index: KeyedRows<(SymbolHandle, SymbolHandle, usize, usize), ContractCallFact>,
    /// Exit-coordinate -> `contract_exits` handles, keyed by
    /// `(machine_symbol, state_symbol, transition_target)`, in arena order.
    pub exit_index: KeyedRows<
        (
            SymbolHandle,
            SymbolHandle,
            typed_trees::statement::TransitionTargetHandle,
        ),
        ContractExitFact,
    >,
    /// Statement-coordinate -> `outcome_specific_arms` rows, keyed by
    /// `(caller_machine_symbol, caller_state_symbol, statement_index)`.
    pub outcome_arm_index: KeyedRows<(SymbolHandle, SymbolHandle, usize), OutcomeSpecificArmFact>,
    /// Statement-coordinate -> `proof_output_calls` rows, keyed by
    /// `(caller_machine_symbol, caller_state_symbol, statement_index)`.
    pub proof_output_index: KeyedRows<(SymbolHandle, SymbolHandle, usize), ProofOutputCallFact>,
}

impl ProofFacts {
    /// The last `contract_calls` row recorded under one caller coordinate —
    /// `(machine symbol, state symbol, statement index, call ordinal)`. The
    /// last row recorded under a coordinate wins, matching the linear scan
    /// the grouping replaced; a call is addressed by its recorded identity
    /// rather than its position.
    pub fn contract_call_at(
        &self,
        key: &(SymbolHandle, SymbolHandle, usize, usize),
    ) -> Option<&ContractCallFact> {
        let handle = self.call_index.with_rows(
            &self.contract_calls,
            |call| {
                (
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
            },
            key,
            |rows| rows.last().copied(),
        )?;
        Some(self.contract_calls.get(handle))
    }

    /// The `contract_exits` rows recorded at one exit coordinate
    /// `(machine_symbol, state_symbol, transition_target)`, in arena order.
    pub fn contract_exits_at(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        transition_target: typed_trees::statement::TransitionTargetHandle,
    ) -> Vec<Handle<ContractExitFact>> {
        self.exit_index.with_rows(
            &self.contract_exits,
            |exit| {
                (
                    exit.machine_symbol,
                    exit.state_symbol,
                    exit.transition_target,
                )
            },
            &(machine_symbol, state_symbol, transition_target),
            |rows| rows.to_vec(),
        )
    }

    /// Whether `outcome_specific_arms` contains any row at `(machine, state, statement_index)`.
    pub fn has_outcome_specific_arm(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) -> bool {
        self.outcome_arm_index.with_rows(
            &self.outcome_specific_arms,
            |arm| {
                (
                    arm.caller_machine_symbol,
                    arm.caller_state_symbol,
                    arm.statement_index,
                )
            },
            &(machine_symbol, state_symbol, statement_index),
            |rows| !rows.is_empty(),
        )
    }

    /// Whether `proof_output_calls` contains any proof-only call (`runtime_call.is_none()`)
    /// at `(machine, state, statement_index)`.
    pub fn has_proof_only_output_call(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
    ) -> bool {
        self.proof_output_index.with_rows(
            &self.proof_output_calls,
            |invocation| {
                (
                    invocation.caller_machine_symbol,
                    invocation.caller_state_symbol,
                    invocation.statement_index,
                )
            },
            &(machine_symbol, state_symbol, statement_index),
            |rows| {
                rows.iter()
                    .any(|handle| self.proof_output_calls.get(*handle).runtime_call.is_none())
            },
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

impl ProofFacts {
    pub fn integer_assignment_at(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: usize,
        target: typed_trees::expression::ExpressionHandle,
        value: typed_trees::expression::ExpressionHandle,
    ) -> Handle<crate::CheckedIntegerAssignmentDecision> {
        let mut matches = self.integer_assignments.iter().filter(|(_, decision)| {
            decision.machine == machine
                && decision.state == state
                && decision.statement_index == statement_index
                && decision.target == target
                && decision.value == value
        });
        let Some((handle, _)) = matches.next() else {
            return Handle::invalid();
        };
        if matches.next().is_some() {
            Handle::invalid()
        } else {
            handle
        }
    }
}
