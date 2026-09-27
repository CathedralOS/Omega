//! Proposition, contract, certificate and evidence lowering.
//!
//! Owns crash routes and canonical propositions, logical contract conversion,
//! content conservation guarantees, float-meaning projections, quotient
//! correspondences, proof-SCC custody, scalar block invariants, the focused
//! integer certificate producer, ranking certificates, evidence artifacts,
//! mathematical declaration admission, and operation proof completion over the
//! finished module.

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedPropositionBinderArgumentKind,
    CheckedPropositionBinderKind, CheckedPropositionEvidence, CheckedScalarExpression,
    CheckedTrees, CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, ContentIdentityReshuffleFact,
    ContentPartitionCompositionFact,
};
use language_semantics::content::{
    ContentAlgebraIdentity as CheckedContentAlgebraIdentity, ContentArithmeticOperator,
    ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
    ContentConservationTerm as CheckedContentConservationTerm,
    ContentPlaceRoot as CheckedContentPlaceRoot, ContentPlaceSegment as CheckedContentPlaceSegment,
    ContentPlaceVersion as CheckedContentPlaceVersion,
    ContentProjectionExpression as CheckedContentProjectionExpression,
    ContentScalarExpression as CheckedContentScalarExpression,
    ContentStructuralPlace as CheckedContentStructuralPlace, conservation_report_fingerprint,
};
use language_semantics::{PermissionClaimIdentity, SemanticDomainId};
use lowered_psi::LoweredPsi;
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ClaimId, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm, EvidenceIdentity, EvidenceTermId,
    IeeeFloatFormat, IeeeFloatStructuralField, IntegerSign, IntegerType, IntegerValue, MachineId,
    PlaceId, Proposition, PropositionContext, PropositionId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    BoundaryContentGuarantee, ClaimContentProjection, ContentConservationGuarantee,
    ContentEntryClaim, ContentIdentityReshuffle, ContentPlaceSubstitution,
    CrashCause as TerminalCrashCause, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    EvidenceTermDeclaration, OperationKind, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificEvidenceUse,
    OutcomeSpecificGuard, ProofOutput, ProofOutputCall, ProofOutputRuntimeCall,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, StaticRequirementDispatch,
    StructuralContentProjection, StructuralFieldType, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
    ValueDeclaration,
};
use terminal_verifier::{
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence,
};

use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_scalar_type, integer_value, terminal_scalar_type,
};
use crate::expression_preparation::prepare_expression::lower_checked_scalar_expression;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::{
    dense_identity, lookup_claim_id, machine_id, obligation_id, proposition_id, value_id,
};

pub(crate) mod content_conservation;
pub(crate) mod contract_predicates;
pub(crate) mod control_cycle_proofs;
pub(crate) mod crash_routes;
pub(crate) mod entry_requirement_certificates;
pub(crate) mod evidence_lowering;
pub(crate) mod float_meaning_projection;
pub(crate) mod mathematical_declarations;
pub(crate) mod nonzero_divisor_certificate;
pub(crate) mod operation_proofs;
pub(crate) mod proof_recursion;
pub(crate) mod quotient_correspondence;
pub(crate) mod scalar_block_invariants;

/// Scratch instrumentation for C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT attribution.
/// Enabled by OMEGA_PROOF_TIMINGS=1; prints to stderr.
pub(crate) mod proof_timing {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    pub(crate) fn enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("OMEGA_PROOF_TIMINGS").is_ok())
    }

    pub(crate) struct Scope {
        name: &'static str,
        started: Instant,
    }

    impl Scope {
        pub(crate) fn enter(name: &'static str) -> Option<Self> {
            enabled().then(|| Self {
                name,
                started: Instant::now(),
            })
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            eprintln!(
                "[proof-timing] {} {:.3}ms",
                self.name,
                self.started.elapsed().as_secs_f64() * 1000.0
            );
        }
    }

    pub(crate) static RECONSTRUCT_CALLS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static VALIDATE_CALLS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static RECONSTRUCT_MICROS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static VALIDATE_MICROS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static VALUE_CONTEXT_CALLS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static VALUE_CONTEXT_MICROS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static PRODUCER_CALLS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static PRODUCER_MICROS: AtomicU64 = AtomicU64::new(0);

    pub(crate) fn count_reconstruct(elapsed: std::time::Duration) {
        if enabled() {
            RECONSTRUCT_CALLS.fetch_add(1, Ordering::Relaxed);
            RECONSTRUCT_MICROS.fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn count_validate(elapsed: std::time::Duration) {
        if enabled() {
            VALIDATE_CALLS.fetch_add(1, Ordering::Relaxed);
            VALIDATE_MICROS.fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn count_value_context(elapsed: std::time::Duration) {
        if enabled() {
            VALUE_CONTEXT_CALLS.fetch_add(1, Ordering::Relaxed);
            VALUE_CONTEXT_MICROS.fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn count_producer(elapsed: std::time::Duration) {
        if enabled() {
            PRODUCER_CALLS.fetch_add(1, Ordering::Relaxed);
            PRODUCER_MICROS.fetch_add(elapsed.as_micros() as u64, Ordering::Relaxed);
        }
    }

    /// Counts the enclosing scope's elapsed time into `counter` on drop.
    pub(crate) struct Counted {
        started: Instant,
        counter: fn(std::time::Duration),
    }

    impl Counted {
        pub(crate) fn enter(counter: fn(std::time::Duration)) -> Option<Self> {
            enabled().then(|| Self {
                started: Instant::now(),
                counter,
            })
        }
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            (self.counter)(self.started.elapsed());
        }
    }

    pub(crate) static VERDICT_HITS: AtomicU64 = AtomicU64::new(0);
    pub(crate) static VERDICT_MISSES: AtomicU64 = AtomicU64::new(0);
    pub(crate) static BUDGET_EXHAUSTIONS: AtomicU64 = AtomicU64::new(0);

    /// A cached proof verdict was reused instead of re-running the search.
    /// Counted unconditionally: the accounting must be accurate with or
    /// without the timing flag.
    pub(crate) fn count_verdict_hit() {
        VERDICT_HITS.fetch_add(1, Ordering::Relaxed);
    }

    /// A proof search ran (its duration is counted by `count_producer`).
    /// Counted unconditionally for the same reason.
    pub(crate) fn count_verdict_miss() {
        VERDICT_MISSES.fetch_add(1, Ordering::Relaxed);
    }

    /// A named private proof-work capacity declined a candidate. Bounded
    /// execution keeps the verdict but the exhaustion is accounted here —
    /// unconditionally, so the aggregate is never silent.
    pub(crate) fn count_budget_exhaustion() {
        BUDGET_EXHAUSTIONS.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn report(label: &str) {
        if enabled() {
            eprintln!(
                "[proof-timing] {} reconstruct: {} calls {:.1}ms | validate: {} calls {:.1}ms | value_context: {} calls {:.1}ms | producer: {} calls {:.1}ms | verdicts: {} hits {} misses | budget exhaustions: {}",
                label,
                RECONSTRUCT_CALLS.load(Ordering::Relaxed),
                RECONSTRUCT_MICROS.load(Ordering::Relaxed) as f64 / 1000.0,
                VALIDATE_CALLS.load(Ordering::Relaxed),
                VALIDATE_MICROS.load(Ordering::Relaxed) as f64 / 1000.0,
                VALUE_CONTEXT_CALLS.load(Ordering::Relaxed),
                VALUE_CONTEXT_MICROS.load(Ordering::Relaxed) as f64 / 1000.0,
                PRODUCER_CALLS.load(Ordering::Relaxed),
                PRODUCER_MICROS.load(Ordering::Relaxed) as f64 / 1000.0,
                VERDICT_HITS.load(Ordering::Relaxed),
                VERDICT_MISSES.load(Ordering::Relaxed),
                BUDGET_EXHAUSTIONS.load(Ordering::Relaxed),
            );
        }
    }
}
