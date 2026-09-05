use crate::{CrashCause, EvidenceInterfaceIdentity};
use semantic_vocabulary::{
    ContractId, EvidenceTermId, MachineId, ObligationId, PlaceId, Proposition, PropositionId,
    StructuralCaseId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContract {
    pub id: ContractId,
    /// Strictly ordered canonical may-routes. Omitting a cause forbids it.
    pub crash_routes: Vec<CrashRouteBucket>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<ContractClause>,
    /// Outcome-specific guarantees remain disjoint from unconditional lanes.
    /// Canonical order is `(result_type, result_case, position)`.
    pub outcome_specific_ensures: Vec<OutcomeSpecificEnsure>,
}

/// Exact nominal result-case guard for one semantic guarantee row.
///
/// This is independent from the proposition and evidence-term identities. It
/// authorizes executable matching-exit replay only when Terminal verification
/// independently recognizes an exact case-producing return carrier; wider
/// structural control remains fail closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutcomeSpecificGuard {
    pub result_type: StructuralTypeId,
    pub result_case: StructuralCaseId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificEnsure {
    pub guard: OutcomeSpecificGuard,
    /// Dense zero-based order within one exact result-case group.
    pub position: u32,
    pub obligation: ObligationId,
    pub proposition: Proposition,
    /// Present exactly for a named witness-bearing guarantee.
    pub evidence: Option<OutcomeSpecificEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificEvidence {
    pub term: EvidenceTermId,
    pub output_field: String,
}

/// One caller-local evidence term selected from an exact guarded callee row.
///
/// This carrier is proof-only. The guard remains conditional on the runtime
/// structural result; the binding neither asserts case membership nor adds an
/// operation. A bounded payloadless structural call may retain any selected
/// subset, canonically ordered by guarded row coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificCallEvidence {
    pub guard: OutcomeSpecificGuard,
    pub position: u32,
    pub callee_obligation: ObligationId,
    pub callee_term: EvidenceTermId,
    pub output_field: String,
    /// Exact proposition application declared by the guarded callee row.
    pub callee_proposition: PropositionId,
    /// Exact caller-side application after substituting the call result.
    pub instantiated_proposition: PropositionId,
    pub output: EvidenceTermId,
    /// Present exactly when the proposition application mentions the complete
    /// structural result. This source-handle-free row, rather than application
    /// display strings, authorizes the one bounded substitution.
    pub result_substitution: Option<OutcomeSpecificCallResultSubstitution>,
    pub validity: OutcomeSpecificCallEvidenceValidity,
    /// Independent cardinality commitment for the bounded selected-term use
    /// lane. This keeps omission distinct from an intentionally unused row.
    pub expected_use_count: u32,
    pub uses: Vec<OutcomeSpecificEvidenceUse>,
}

/// One proof-only consumption of a selected guarded term by an exact direct
/// tail-state `requires` position. It adds no runtime edge or fuel unit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutcomeSpecificEvidenceUse {
    pub target: MachineId,
    pub input_position: u32,
    pub target_requirement: PropositionId,
    pub target_term: EvidenceTermId,
    pub source: EvidenceTermId,
    pub instantiated_proposition: PropositionId,
    pub target_parameter: PlaceId,
    pub caller_result: PlaceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutcomeSpecificCallResultSubstitution {
    pub argument_position: u32,
    pub callee_result: PlaceId,
    pub caller_result: PlaceId,
}

/// Source-handle-free roots of the checked guarded-term validity intersection.
///
/// The bounded payloadless call has no arguments or payload projections, so
/// every retained occurrence can name only its exact structural result root.
/// Interface identity is repeated deliberately so codec and verifier replay
/// cannot silently detach validity from the selected witness carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeSpecificCallEvidenceValidity {
    pub result: PlaceId,
    pub proposition_dependencies: Vec<PlaceId>,
    pub evidence_interface: EvidenceInterfaceIdentity,
    pub interface_dependencies: Vec<PlaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucket {
    pub cause: CrashCause,
    /// Canonical nonempty disjunction. `Truth`, when present, is the sole row.
    pub alternatives: Vec<CrashRouteGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashRouteGuard {
    Truth,
    Predicate(CrashPredicateTerm),
}

/// Canonical source-independent term for one normalized crash predicate.
///
/// Terminal Psi retains the proposition itself. The verifier can therefore
/// type-check it, substitute callee values at a call, and reconstruct the exact
/// surviving continuation without trusting producer-authored identity bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashPredicateTerm(Proposition);

impl CrashPredicateTerm {
    pub const fn new(proposition: Proposition) -> Self {
        Self(proposition)
    }

    pub const fn proposition(&self) -> &Proposition {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractClause {
    pub obligation: ObligationId,
    pub proposition: Proposition,
}
