use crate::{CheckedValueOrigin, CrashCause};
use psi_arena::{Arena, Handle, HandleSpan};
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_symbols::SymbolHandle;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::signature::SignatureContract;
use psi_typed_trees::types::TypeReferenceHandle;

pub use psi_numerics::arithmetic::ArithmeticPolicyAdapter as CheckedArithmeticPolicyAdapter;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CheckedOperatorResolutionStatus {
    #[default]
    Missing,
    Resolved,
    Ambiguous,
    /// Recorded before flow facts exist: the use carries domain-owned spelled
    /// candidates whose admissibility depends on the proof context at the use
    /// site (chapter 8: only PROVEN domains participate in operator
    /// resolution). The post-flow selection pass rewrites this into one of the
    /// final statuses; it must never survive to the checks.
    DomainPending,
    /// Final: every domain-owned candidate was inadmissible (its domain
    /// membership is not proven at the use site) and the operand type carries
    /// the ordinary builtin meaning, so the builtin operation stays selected
    /// (chapter 8: the ordinary integer `+` is not automatically replaced).
    BuiltinFallback,
    /// Final: domain-owned candidates exist but none is admissible (no proven
    /// domain fact) and no root or builtin meaning exists for the operand
    /// type. The use has no admissible meaning and must be rejected.
    Inadmissible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedOperatorUseFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    pub spelling: OperatorSpelling,
    pub policy_adapter: CheckedArithmeticPolicyAdapter,
    /// Exact selected ProviderPlan identity for a migrated boundary-operator
    /// realization. Zero means the operator still uses bootstrap lowering.
    pub provider_plan_identity: u64,
    pub selected_operator_symbol: SymbolHandle,
    pub candidates: HandleSpan<CheckedOperatorCandidateFact>,
    pub candidate_count: usize,
    pub status: CheckedOperatorResolutionStatus,
}

/// One uniquely resolved named operator call retained as checked evidence.
/// Named calls have no spelling-resolution candidate set, but they still need
/// the selected semantic identity and any arithmetic-policy result adapter to
/// survive past typed trees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedNamedOperatorUseFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    pub selected_operator_symbol: SymbolHandle,
    pub policy_adapter: CheckedArithmeticPolicyAdapter,
    pub provider_plan_identity: u64,
}

impl Default for CheckedOperatorUseFact {
    fn default() -> Self {
        Self {
            expression: ExpressionHandle::invalid(),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            policy_adapter: CheckedArithmeticPolicyAdapter::None,
            provider_plan_identity: 0,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: HandleSpan::empty(),
            candidate_count: 0,
            status: CheckedOperatorResolutionStatus::Missing,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedOperatorCandidateFact {
    pub operator_symbol: SymbolHandle,
    pub domain_symbol: SymbolHandle,
    /// Exact proof-static conformance declaration selected for a trait-owned
    /// fixed token. Invalid for root/domain meanings.
    pub conformance_symbol: SymbolHandle,
    /// Trait requirement whose fixed token this candidate realizes.
    pub trait_requirement_symbol: SymbolHandle,
    /// Exact closed requirement-map realization retained by the selected
    /// conformance row.
    pub realization_machine_symbol: SymbolHandle,
    pub realization_state_symbol: SymbolHandle,
    /// Compact report coordinate; authority uses the adjacent commitment.
    pub conformance_application_report_fingerprint: u64,
    pub conformance_application_commitment:
        psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
    pub receiver_type: TypeReferenceHandle,
    pub return_type: TypeReferenceHandle,
    pub contracts: HandleSpan<SignatureContract>,
    pub type_parameter_count: usize,
    pub parameter_count: usize,
    pub contract_count: usize,
    pub is_boundary: bool,
}

impl CheckedOperatorCandidateFact {
    pub const fn root(operator_symbol: SymbolHandle) -> Self {
        Self {
            operator_symbol,
            domain_symbol: SymbolHandle::invalid(),
            conformance_symbol: SymbolHandle::invalid(),
            trait_requirement_symbol: SymbolHandle::invalid(),
            realization_machine_symbol: SymbolHandle::invalid(),
            realization_state_symbol: SymbolHandle::invalid(),
            conformance_application_report_fingerprint: 0,
            conformance_application_commitment:
                psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
                    [0; 32],
                ),
            receiver_type: TypeReferenceHandle::invalid(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            type_parameter_count: 0,
            parameter_count: 0,
            contract_count: 0,
            is_boundary: false,
        }
    }

    pub const fn domain(operator_symbol: SymbolHandle, domain_symbol: SymbolHandle) -> Self {
        Self {
            operator_symbol,
            domain_symbol,
            conformance_symbol: SymbolHandle::invalid(),
            trait_requirement_symbol: SymbolHandle::invalid(),
            realization_machine_symbol: SymbolHandle::invalid(),
            realization_state_symbol: SymbolHandle::invalid(),
            conformance_application_report_fingerprint: 0,
            conformance_application_commitment:
                psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment::from_digest(
                    [0; 32],
                ),
            receiver_type: TypeReferenceHandle::invalid(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            type_parameter_count: 0,
            parameter_count: 0,
            contract_count: 0,
            is_boundary: false,
        }
    }

    pub const fn is_domain_owned(self) -> bool {
        self.domain_symbol.is_valid()
    }

    pub const fn trait_backed(
        requirement_symbol: SymbolHandle,
        conformance_symbol: SymbolHandle,
        realization_machine_symbol: SymbolHandle,
        realization_state_symbol: SymbolHandle,
        conformance_application_report_fingerprint: u64,
        conformance_application_commitment:
            psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment,
    ) -> Self {
        Self {
            operator_symbol: requirement_symbol,
            domain_symbol: SymbolHandle::invalid(),
            conformance_symbol,
            trait_requirement_symbol: requirement_symbol,
            realization_machine_symbol,
            realization_state_symbol,
            conformance_application_report_fingerprint,
            conformance_application_commitment,
            receiver_type: TypeReferenceHandle::invalid(),
            return_type: TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
            type_parameter_count: 0,
            parameter_count: 0,
            contract_count: 0,
            is_boundary: false,
        }
    }

    pub const fn is_trait_backed(self) -> bool {
        self.conformance_symbol.is_valid() && self.trait_requirement_symbol.is_valid()
    }

    pub const fn with_signature(
        mut self,
        receiver_type: TypeReferenceHandle,
        return_type: TypeReferenceHandle,
        contracts: HandleSpan<SignatureContract>,
        type_parameter_count: usize,
        parameter_count: usize,
        is_boundary: bool,
    ) -> Self {
        self.receiver_type = receiver_type;
        self.return_type = return_type;
        self.contracts = contracts;
        self.type_parameter_count = type_parameter_count;
        self.parameter_count = parameter_count;
        self.contract_count = contracts.len();
        self.is_boundary = is_boundary;
        self
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedOperatorResolutionSummary {
    pub resolved: usize,
    pub missing: usize,
    pub ambiguous: usize,
    pub builtin_fallback: usize,
    pub inadmissible: usize,
    pub domain_pending: usize,
}

impl CheckedOperatorResolutionSummary {
    pub const fn all_resolved(self) -> bool {
        self.missing == 0
            && self.ambiguous == 0
            && self.inadmissible == 0
            && self.domain_pending == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedOperatorResolutionIssue<'facts> {
    pub operator_use: &'facts CheckedOperatorUseFact,
    pub candidates: &'facts [CheckedOperatorCandidateFact],
}

impl CheckedOperatorResolutionIssue<'_> {
    pub const fn status(&self) -> CheckedOperatorResolutionStatus {
        self.operator_use.status
    }

    pub const fn is_missing(&self) -> bool {
        matches!(
            self.operator_use.status,
            CheckedOperatorResolutionStatus::Missing
        )
    }

    pub const fn is_ambiguous(&self) -> bool {
        matches!(
            self.operator_use.status,
            CheckedOperatorResolutionStatus::Ambiguous
        )
    }

    pub const fn is_inadmissible(&self) -> bool {
        matches!(
            self.operator_use.status,
            CheckedOperatorResolutionStatus::Inadmissible
        )
    }

    pub const fn spelling(&self) -> OperatorSpelling {
        self.operator_use.spelling
    }

    pub const fn candidate_count(&self) -> usize {
        self.operator_use.candidate_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedOperatorContractUse<'facts> {
    pub operator_use: &'facts CheckedOperatorUseFact,
    pub candidate: &'facts CheckedOperatorCandidateFact,
}

impl CheckedOperatorContractUse<'_> {
    pub const fn contract_count(&self) -> usize {
        self.candidate.contract_count
    }

    pub const fn contracts(&self) -> HandleSpan<SignatureContract> {
        self.candidate.contracts
    }

    pub const fn operator_symbol(&self) -> SymbolHandle {
        self.candidate.operator_symbol
    }

    pub const fn domain_symbol(&self) -> SymbolHandle {
        self.candidate.domain_symbol
    }

    pub const fn is_boundary(&self) -> bool {
        self.candidate.is_boundary
    }
}

/// One checked, cause-normalized crash bucket on an operator declaration.
/// Fact handles are compiler-private joins back into typed Psi; package review
/// projects their exact structural identity only after checking succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedOperatorCrashBucket {
    cause: CrashCause,
    unconditional: bool,
    facts: Vec<Handle<ProofFact>>,
}

impl CheckedOperatorCrashBucket {
    pub fn new(cause: CrashCause, unconditional: bool, facts: Vec<Handle<ProofFact>>) -> Self {
        Self {
            cause,
            unconditional,
            facts,
        }
    }

    pub const fn cause(&self) -> CrashCause {
        self.cause
    }

    pub const fn is_unconditional(&self) -> bool {
        self.unconditional
    }

    pub fn facts(&self) -> &[Handle<ProofFact>] {
        &self.facts
    }
}

/// Complete checked crash surface for one exact operator declaration. One row
/// exists even when the operator publishes no crash routes, so absence cannot
/// be confused with an empty ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedOperatorCrashContract {
    operator_symbol: SymbolHandle,
    buckets: Vec<CheckedOperatorCrashBucket>,
}

impl CheckedOperatorCrashContract {
    pub fn new(operator_symbol: SymbolHandle, buckets: Vec<CheckedOperatorCrashBucket>) -> Self {
        Self {
            operator_symbol,
            buckets,
        }
    }

    pub const fn operator_symbol(&self) -> SymbolHandle {
        self.operator_symbol
    }

    pub fn buckets(&self) -> &[CheckedOperatorCrashBucket] {
        &self.buckets
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedOperatorFacts {
    pub uses: Arena<CheckedOperatorUseFact>,
    pub named_uses: Arena<CheckedNamedOperatorUseFact>,
    pub candidates: Arena<CheckedOperatorCandidateFact>,
    pub operator_crash_contracts: Vec<CheckedOperatorCrashContract>,
    pub operator_realization_contracts: Vec<CheckedOperatorRealizationContract>,
}

/// Retained checked baseline for one machine-to-operator realization.
///
/// Admission shape, canonical contract encodings, and exact typed semantic
/// snapshots are retained in full rather than represented by a hash. Package
/// review can therefore rederive the association from the final typed graph and
/// require exact equality without relying only on mutable proof-fact handles or
/// a collision-prone digest. Trusted compiler components remain inside the TCB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedOperatorRealizationContract {
    machine_symbol: SymbolHandle,
    operator_symbol: SymbolHandle,
    provider_contracts: Vec<Vec<u8>>,
    requirement_contracts: Vec<Vec<u8>>,
    provider_contract_snapshot: Vec<u8>,
    requirement_contract_snapshot: Vec<u8>,
    admission_snapshot: Vec<u8>,
}

impl CheckedOperatorRealizationContract {
    pub fn new(
        machine_symbol: SymbolHandle,
        operator_symbol: SymbolHandle,
        provider_contracts: Vec<Vec<u8>>,
        requirement_contracts: Vec<Vec<u8>>,
        provider_contract_snapshot: Vec<u8>,
        requirement_contract_snapshot: Vec<u8>,
        admission_snapshot: Vec<u8>,
    ) -> Self {
        Self {
            machine_symbol,
            operator_symbol,
            provider_contracts,
            requirement_contracts,
            provider_contract_snapshot,
            requirement_contract_snapshot,
            admission_snapshot,
        }
    }

    pub const fn machine_symbol(&self) -> SymbolHandle {
        self.machine_symbol
    }

    pub const fn operator_symbol(&self) -> SymbolHandle {
        self.operator_symbol
    }

    pub fn provider_contracts(&self) -> &[Vec<u8>] {
        &self.provider_contracts
    }

    pub fn requirement_contracts(&self) -> &[Vec<u8>] {
        &self.requirement_contracts
    }

    pub fn provider_contract_snapshot(&self) -> &[u8] {
        &self.provider_contract_snapshot
    }

    pub fn requirement_contract_snapshot(&self) -> &[u8] {
        &self.requirement_contract_snapshot
    }

    pub fn admission_snapshot(&self) -> &[u8] {
        &self.admission_snapshot
    }
}

impl CheckedOperatorFacts {
    pub fn with_roots(
        uses: Arena<CheckedOperatorUseFact>,
        named_uses: Arena<CheckedNamedOperatorUseFact>,
        candidates: Arena<CheckedOperatorCandidateFact>,
    ) -> Self {
        Self {
            uses,
            named_uses,
            candidates,
            operator_crash_contracts: Vec::new(),
            operator_realization_contracts: Vec::new(),
        }
    }

    pub fn with_operator_crash_contracts(
        mut self,
        operator_crash_contracts: Vec<CheckedOperatorCrashContract>,
    ) -> Self {
        self.operator_crash_contracts = operator_crash_contracts;
        self
    }

    pub fn with_operator_realization_contracts(
        mut self,
        operator_realization_contracts: Vec<CheckedOperatorRealizationContract>,
    ) -> Self {
        self.operator_realization_contracts = operator_realization_contracts;
        self
    }

    pub fn expression_use(&self, expression: ExpressionHandle) -> Option<&CheckedOperatorUseFact> {
        self.uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression).then_some(operator_use)
        })
    }

    pub fn expression_use_in_origin(
        &self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<&CheckedOperatorUseFact> {
        self.uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression && operator_use.origin == origin)
                .then_some(operator_use)
        })
    }

    pub fn uses_with_status(
        &self,
        status: CheckedOperatorResolutionStatus,
    ) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses.iter().filter_map(move |(_, operator_use)| {
            (operator_use.status == status).then_some(operator_use)
        })
    }

    pub fn resolved_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses_with_status(CheckedOperatorResolutionStatus::Resolved)
    }

    pub fn named_uses(&self) -> impl Iterator<Item = &CheckedNamedOperatorUseFact> {
        self.named_uses.iter().map(|(_, operator_use)| operator_use)
    }

    pub fn named_expression_use_in_origin(
        &self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<&CheckedNamedOperatorUseFact> {
        self.named_uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression && operator_use.origin == origin)
                .then_some(operator_use)
        })
    }

    /// One query for downstream consumers that do not care whether the source
    /// selected a spelled or named operator identity.
    pub fn policy_adapter_evidence_for_expression_in_origin(
        &self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<CheckedArithmeticPolicyAdapter> {
        self.expression_use_in_origin(expression, origin)
            .map(|operator_use| operator_use.policy_adapter)
            .or_else(|| {
                self.named_expression_use_in_origin(expression, origin)
                    .map(|operator_use| operator_use.policy_adapter)
            })
    }

    /// Compatibility query for callers that only need the adapter value and
    /// deliberately treat missing operator evidence as no adapter.
    pub fn policy_adapter_for_expression_in_origin(
        &self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> CheckedArithmeticPolicyAdapter {
        self.policy_adapter_evidence_for_expression_in_origin(expression, origin)
            .unwrap_or_default()
    }

    pub fn provider_plan_identity_for_expression_in_origin(
        &self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
    ) -> Option<u64> {
        self.expression_use_in_origin(expression, origin)
            .map(|operator_use| operator_use.provider_plan_identity)
            .or_else(|| {
                self.named_expression_use_in_origin(expression, origin)
                    .map(|operator_use| operator_use.provider_plan_identity)
            })
            .filter(|identity| *identity != 0)
    }

    pub fn missing_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses_with_status(CheckedOperatorResolutionStatus::Missing)
    }

    pub fn ambiguous_uses(&self) -> impl Iterator<Item = &CheckedOperatorUseFact> {
        self.uses_with_status(CheckedOperatorResolutionStatus::Ambiguous)
    }

    pub fn resolution_issues(&self) -> impl Iterator<Item = CheckedOperatorResolutionIssue<'_>> {
        self.uses.iter().filter_map(|(_, operator_use)| {
            matches!(
                operator_use.status,
                CheckedOperatorResolutionStatus::Missing
                    | CheckedOperatorResolutionStatus::Ambiguous
                    | CheckedOperatorResolutionStatus::Inadmissible
            )
            .then(|| CheckedOperatorResolutionIssue {
                operator_use,
                candidates: self.candidates(operator_use),
            })
        })
    }

    /// The candidate fact carrying the selected operator symbol of a resolved
    /// use, when one was selected. Multi-candidate uses keep their full
    /// considered set as evidence; the selected symbol names the winner.
    pub fn selected_candidate(
        &self,
        operator_use: &CheckedOperatorUseFact,
    ) -> Option<&CheckedOperatorCandidateFact> {
        if !operator_use.selected_operator_symbol.is_valid() {
            return None;
        }
        self.candidates(operator_use)
            .iter()
            .find(|candidate| candidate.operator_symbol == operator_use.selected_operator_symbol)
    }

    /// Exact trait-backed realization selected for this expression while one
    /// machine instance executes. Nested value facts may repeat an expression
    /// without an owning machine; they never become a second authority.
    pub fn selected_trait_candidate_in_machine(
        &self,
        expression: ExpressionHandle,
        machine_symbol: SymbolHandle,
    ) -> Option<&CheckedOperatorCandidateFact> {
        self.uses.iter().find_map(|(_, operator_use)| {
            (operator_use.expression == expression
                && operator_use.origin.machine_symbol() == Some(machine_symbol)
                && operator_use.status == CheckedOperatorResolutionStatus::Resolved)
                .then(|| self.selected_candidate(operator_use))
                .flatten()
                .filter(|candidate| candidate.is_trait_backed())
        })
    }

    pub fn resolved_contract_uses(&self) -> impl Iterator<Item = CheckedOperatorContractUse<'_>> {
        self.resolved_uses().filter_map(|operator_use| {
            let [candidate] = self.candidates(operator_use) else {
                return None;
            };
            (candidate.contract_count > 0).then_some(CheckedOperatorContractUse {
                operator_use,
                candidate,
            })
        })
    }

    pub fn candidates(
        &self,
        operator_use: &CheckedOperatorUseFact,
    ) -> &[CheckedOperatorCandidateFact] {
        self.candidates.span_or_empty(operator_use.candidates)
    }

    pub fn candidate_symbols(
        &self,
        operator_use: &CheckedOperatorUseFact,
    ) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.candidates(operator_use)
            .iter()
            .map(|candidate| candidate.operator_symbol)
    }

    pub fn resolution_summary(&self) -> CheckedOperatorResolutionSummary {
        let mut summary = CheckedOperatorResolutionSummary::default();
        for (_, operator_use) in self.uses.iter() {
            match operator_use.status {
                CheckedOperatorResolutionStatus::Resolved => {
                    summary.resolved = summary.resolved.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::Missing => {
                    summary.missing = summary.missing.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::Ambiguous => {
                    summary.ambiguous = summary.ambiguous.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::BuiltinFallback => {
                    summary.builtin_fallback = summary.builtin_fallback.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::Inadmissible => {
                    summary.inadmissible = summary.inadmissible.saturating_add(1);
                }
                CheckedOperatorResolutionStatus::DomainPending => {
                    summary.domain_pending = summary.domain_pending.saturating_add(1);
                }
            }
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_numerics::float_semantics::FloatFormat;

    #[test]
    fn checked_operator_facts_constructor_keeps_use_root_explicit() {
        let mut candidates = Arena::with_capacity(3);
        let receiver_type = TypeReferenceHandle::from_arena_index(8);
        let return_type = TypeReferenceHandle::from_arena_index(9);
        let contracts = HandleSpan::from_parts(
            psi_arena::Handle::<SignatureContract>::from_arena_index(20),
            3,
        );
        let resolved_candidates = candidates.insert_many([CheckedOperatorCandidateFact::root(
            SymbolHandle::from_arena_index(2),
        )
        .with_signature(receiver_type, return_type, contracts, 1, 2, true)]);
        let ambiguous_candidates = candidates.insert_many([
            CheckedOperatorCandidateFact::root(SymbolHandle::from_arena_index(4)),
            CheckedOperatorCandidateFact::domain(
                SymbolHandle::from_arena_index(5),
                SymbolHandle::from_arena_index(6),
            ),
        ]);

        let mut uses = Arena::with_capacity(2);
        let expression = ExpressionHandle::from_arena_index(1);
        uses.append(CheckedOperatorUseFact {
            expression,
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            policy_adapter: CheckedArithmeticPolicyAdapter::None,
            provider_plan_identity: 0,
            selected_operator_symbol: SymbolHandle::from_arena_index(2),
            candidates: resolved_candidates,
            candidate_count: 1,
            status: CheckedOperatorResolutionStatus::Resolved,
        });
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(3),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Range,
            policy_adapter: CheckedArithmeticPolicyAdapter::None,
            provider_plan_identity: 0,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: ambiguous_candidates,
            candidate_count: 2,
            status: CheckedOperatorResolutionStatus::Ambiguous,
        });

        let mut named_uses = Arena::with_capacity(1);
        let named_expression = ExpressionHandle::from_arena_index(7);
        named_uses.append(CheckedNamedOperatorUseFact {
            expression: named_expression,
            origin: CheckedValueOrigin::default(),
            selected_operator_symbol: SymbolHandle::from_arena_index(8),
            policy_adapter: CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                format: FloatFormat::BINARY64,
            },
            provider_plan_identity: 0,
        });

        let facts =
            CheckedOperatorFacts::with_roots(uses.clone(), named_uses.clone(), candidates.clone());

        assert_eq!(facts.uses, uses);
        assert_eq!(facts.named_uses, named_uses);
        assert_eq!(facts.candidates, candidates);
        assert_eq!(facts.named_uses().count(), 1);
        assert_eq!(
            facts.policy_adapter_for_expression_in_origin(
                named_expression,
                CheckedValueOrigin::default(),
            ),
            CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                format: FloatFormat::BINARY64,
            }
        );
        assert_eq!(
            facts
                .expression_use(expression)
                .map(|operator_use| operator_use.status),
            Some(CheckedOperatorResolutionStatus::Resolved)
        );
        assert_eq!(
            facts.resolution_summary(),
            CheckedOperatorResolutionSummary {
                resolved: 1,
                missing: 0,
                ambiguous: 1,
                builtin_fallback: 0,
                inadmissible: 0,
                domain_pending: 0,
            }
        );
        assert!(!facts.resolution_summary().all_resolved());
        assert_eq!(facts.resolved_uses().count(), 1);
        assert_eq!(facts.ambiguous_uses().count(), 1);
        assert_eq!(facts.missing_uses().count(), 0);
        let issues = facts.resolution_issues().collect::<Vec<_>>();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].is_ambiguous());
        assert!(!issues[0].is_missing());
        assert_eq!(
            issues[0].status(),
            CheckedOperatorResolutionStatus::Ambiguous
        );
        assert_eq!(issues[0].spelling(), OperatorSpelling::Range);
        assert_eq!(issues[0].candidate_count(), 2);
        assert_eq!(issues[0].candidates.len(), 2);
        let ambiguous_use = facts.ambiguous_uses().next().expect("ambiguous use");
        assert_eq!(facts.candidates(ambiguous_use).len(), 2);
        let resolved_use = facts.resolved_uses().next().expect("resolved use");
        let resolved_candidate = facts.candidates(resolved_use)[0];
        assert_eq!(resolved_candidate.receiver_type, receiver_type);
        assert_eq!(resolved_candidate.return_type, return_type);
        assert_eq!(resolved_candidate.contracts, contracts);
        assert_eq!(resolved_candidate.type_parameter_count, 1);
        assert_eq!(resolved_candidate.parameter_count, 2);
        assert_eq!(resolved_candidate.contract_count, 3);
        assert!(resolved_candidate.is_boundary);
        let contract_uses = facts.resolved_contract_uses().collect::<Vec<_>>();
        assert_eq!(contract_uses.len(), 1);
        assert_eq!(
            contract_uses[0].operator_use.selected_operator_symbol,
            SymbolHandle::from_arena_index(2)
        );
        assert_eq!(
            contract_uses[0].operator_symbol(),
            SymbolHandle::from_arena_index(2)
        );
        assert_eq!(contract_uses[0].domain_symbol(), SymbolHandle::invalid());
        assert_eq!(contract_uses[0].contract_count(), 3);
        assert_eq!(contract_uses[0].contracts(), contracts);
        assert!(contract_uses[0].is_boundary());
        assert_eq!(
            facts.candidate_symbols(ambiguous_use).collect::<Vec<_>>(),
            vec![
                SymbolHandle::from_arena_index(4),
                SymbolHandle::from_arena_index(5),
            ]
        );
        assert!(facts.candidates(ambiguous_use)[1].is_domain_owned());
    }
}
