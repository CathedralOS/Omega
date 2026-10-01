//! Lowering failure vocabulary shared by every producer in this crate.
//!
//! Unsupported constructs fail closed with a static reason; structured
//! variants retain the identity a caller needs to report or test the refusal.

use language_semantics::PermissionClaimIdentity;
use semantic_vocabulary::{
    ContentProjectionIdentity, ObligationId, PlaceId, PropositionError, StructuralPlaceKind,
};

use crate::proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;

/// A named private capacity of the optional proof-search passes. Each
/// variant names one resource the passes provision privately — an aggregate
/// round count, a shared work pool, or an identity space — so a decline can
/// publish the resource it drew against instead of reading as a disproof of
/// the module. Each variant is one scalar-resource row of the
/// compiler-request `Incomplete` outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofSearchCapacity {
    /// The scalar block-invariant roster pass's aggregate whole-module
    /// discovery and proof rounds (`proof_search_roster_rounds`).
    ScalarInvariantRosterRounds,
    /// The shared candidate-transport pool the roster pass's field-bound
    /// and join producers draw from (`proof_search_candidate_work`).
    ScalarInvariantCandidateWork,
    /// Scope-row visits across one complete canonical or relaxed integer
    /// certificate search, including nested implication and endpoint queries.
    IntegerCertificateSearchWork,
    /// The separate transport pool of the cyclic-guarantee strengthening
    /// pass (`proof_search_cyclic_guarantee_work`).
    CyclicGuaranteeCandidateWork,
    /// Candidate discovery and equation transport for optional scalar result
    /// relations (`proof_search_result_guarantee_work`).
    ScalarResultGuaranteeWork,
    /// Fresh obligation identities one optional pass mints above the
    /// module's registered identities for its proposals' arrivals and
    /// guarantee clauses (`proof_obligation_identities`). A mint past the
    /// provision, or past the end of the nonzero `u64` identity space,
    /// declines.
    TerminalObligationIdentities,
    /// The nonzero-divisor certificate's direct-add conjunction search:
    /// shared definition-DAG visits across one obligation
    /// (`proof_search_divisor_definition_visits`).
    NonzeroDivisorDefinitionVisits,
    /// The same search's nested operand derivation depth
    /// (`proof_search_divisor_search_depth`).
    NonzeroDivisorSearchDepth,
    /// The same search's add-of-adds certificate joins — each join nests
    /// one bound certificate, so the count bounds derivation stacking
    /// (`proof_search_divisor_computed_joins`).
    NonzeroDivisorComputedJoins,
}

impl ProofSearchCapacity {
    /// The scalar-resource name under which the compiler-request outcome
    /// table publishes this capacity's declines.
    pub const fn resource_name(self) -> &'static str {
        match self {
            Self::ScalarInvariantRosterRounds => "proof_search_roster_rounds",
            Self::ScalarInvariantCandidateWork => "proof_search_candidate_work",
            Self::IntegerCertificateSearchWork => "proof_search_integer_certificate_work",
            Self::CyclicGuaranteeCandidateWork => "proof_search_cyclic_guarantee_work",
            Self::ScalarResultGuaranteeWork => "proof_search_result_guarantee_work",
            Self::TerminalObligationIdentities => "proof_obligation_identities",
            Self::NonzeroDivisorDefinitionVisits => "proof_search_divisor_definition_visits",
            Self::NonzeroDivisorSearchDepth => "proof_search_divisor_search_depth",
            Self::NonzeroDivisorComputedJoins => "proof_search_divisor_computed_joins",
        }
    }

    /// The provisioned bound the pass declined against; the producers keep
    /// the same numbers as their internal budgets.
    pub const fn limit(self) -> u64 {
        match self {
            Self::ScalarInvariantRosterRounds => 128,
            // The elapsed-timer composition presents over 11,000 axiom rows
            // before per-header transports and repeated discovery rounds;
            // 16,384 exhausts with its clamp-result bound still unmet. This provisions
            // discovery only; every retained predicate still needs evidence.
            Self::ScalarInvariantCandidateWork => 65_536,
            Self::IntegerCertificateSearchWork => 65_536,
            Self::CyclicGuaranteeCandidateWork => 4096,
            Self::ScalarResultGuaranteeWork => 4096,
            // Each minted identity is one arrival or clause the pass must
            // then prove; the roster pass's candidate pool bounds how many
            // it can propose far below this.
            Self::TerminalObligationIdentities => 65_536,
            Self::NonzeroDivisorDefinitionVisits => 128,
            Self::NonzeroDivisorSearchDepth => 32,
            Self::NonzeroDivisorComputedJoins => 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    MachineNotFound(String),
    AmbiguousMachineName(String),
    DebugSourceFileCountOverflow,
    DebugSourceLengthOverflow,
    MissingDebugSourceFile(usize),
    DebugSemanticCodec(terminal_codec::CodecError),
    InvalidDebugMap(terminal_codec::DebugMapError),
    InvalidTerminalModule(terminal_verifier::ModuleError),
    /// The producer's own replay could not discharge a reconstructed crash
    /// obligation — no supplied certificate answers the question, so the
    /// roster is refused rather than emitted for receivers to reject.
    UndischargedCrashObligations(Vec<terminal_psi::CrashObligationOwner>),
    InvalidFloatMeaningProjection(FloatMeaningProjectionLoweringError),
    InvalidQuotientCorrespondence(Vec<String>),
    OperationProofUnavailable(ObligationId),
    /// An obligation stayed unmet after a named proof-search capacity
    /// declined during this finalization's optional inference — the
    /// publication contract's `Incomplete` outcome, a bounded search that
    /// refused, rather than the ordinary `OperationProofUnavailable`
    /// rejection that asserts nothing was left to find. `requested` is the
    /// declined draw's total demand on `capacity` and `obligation` names the
    /// unproved site; the refusal still prevents `checked` publication
    /// exactly as any other lowering error does.
    ProofSearchCapacityExhausted {
        capacity: ProofSearchCapacity,
        requested: u64,
        obligation: ObligationId,
    },
    InvalidUnitMachinePlan {
        machine: String,
        reason: &'static str,
        /// Why the checked stage left the named machine (or the machine its
        /// closure reached) without a Unit plan, rendered from the checked
        /// record's omission roster when that roster names it.
        omission: Option<String>,
    },
    Unsupported(&'static str),
    /// A borrowed-storage restoration window the checked facts do not pin to
    /// one exact place, or whose restoration is missing on a path. `place` is
    /// the authored-facing spelling of the moved or stored place.
    UnpinnedBorrowedStorageWindow {
        place: String,
        reason: &'static str,
    },
    /// The checked program carries a `Quotient::define`/`Quotient::lift`
    /// request outside the batch the proof-only correspondence extractor
    /// admits; no partial table is retained. Each string is one extraction
    /// diagnostic naming the failed join.
    UnadmittedQuotientRequest {
        diagnostics: Vec<String>,
    },
    InvalidPsiIntegerType,
    UnlandedIntegerLiteral,
    IntegerLandingMismatch,
    IntegerLiteralOutsideSupportedMagnitude,
    IntegerLiteralOutsidePsiType,
    ContentConservationFingerprintMismatch {
        expected: u64,
        actual: u64,
    },
    ContentIdentityFactOwnerMismatch,
    ContentPartitionFactOwnerMismatch,
    ContentPartitionNotConservation,
    ContentPartitionInputClaimNotLowered,
    ContentPartitionInputClaimBindingMismatch,
    ContentEntryClaimRequiresEntryPlace,
    ContentEntryClaimHasNoProjection,
    ContentEntryClaimMapsMultiplePlaces,
    DuplicateContentEntryClaimInput,
    OverlappingContentEntryClaimInput,
    DuplicateContentPartitionSubstitution,
    DuplicateContentPartitionComposition,
    DuplicateContentPartitionProducerCoordinate,
    ContentPartitionProducerOperationMissing,
    ContentPartitionProducerTargetMismatch,
    ContentPartitionResultRewriteUnsupported,
    ContentPartitionDerivedSourceUnsupported,
    ContentPartitionSubstitutionCoverageMismatch,
    ContentPartitionReplayMismatch,
    UnknownContentClaimIdentity,
    ContentIdentityInputParameterMismatch,
    ContentIdentityNotDirectEquality,
    ContentIdentityProjectionMismatch,
    ContentIdentityDirectionMismatch,
    ContentIdentityRootMismatch,
    ContentIdentityClaimMapsMultiplePlaces,
    DuplicateContentIdentityProjection,
    DuplicateContentIdentityInput,
    DuplicateContentIdentityOutput,
    OverlappingContentIdentityInput,
    OverlappingContentIdentityOutput,
    ContentProjectionAlgebraMismatch(ContentProjectionIdentity),
    CrashFrontierClaimNotLowered(PermissionClaimIdentity),
    InvalidContentDomainIdentity,
    ZeroContentProjectionFingerprint,
    ContentTermNestingTooDeep,
    ConflictingContentPlaceRoot {
        id: PlaceId,
        first: StructuralPlaceKind,
        second: StructuralPlaceKind,
    },
    InvalidContentProposition(PropositionError),
    InvalidCrashPredicate(PropositionError),
}

impl LoweringError {
    /// The `unimplemented:` report when a Unit plan is missing because the
    /// compiler has no route for what the body does, not because the body is
    /// invalid.
    pub fn unimplemented_report(&self) -> Option<&str> {
        match self {
            Self::InvalidUnitMachinePlan {
                omission: Some(omission),
                ..
            } if omission.starts_with("unimplemented:") => Some(omission),
            _ => None,
        }
    }

    /// The named capacity, declined demand and unproved obligation when this
    /// refusal is a proof-search capacity exhaustion — the `Incomplete`
    /// outcome class a compiler-request publication maps — and absent when
    /// the refusal is an ordinary rejection.
    pub fn incomplete_capacity(&self) -> Option<ProofSearchExhaustion> {
        match self {
            Self::ProofSearchCapacityExhausted {
                capacity,
                requested,
                obligation,
            } => Some(ProofSearchExhaustion {
                capacity: *capacity,
                requested: *requested,
                obligation: *obligation,
            }),
            _ => None,
        }
    }
}

/// The accounted facts of one proof-search capacity refusal: the scalar
/// resource that declined, the total demand of the draw it declined (past
/// `capacity.limit()`), and the obligation left unproved — the compiler
/// request's `Incomplete` resource, requested amount and internal-row
/// coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofSearchExhaustion {
    pub capacity: ProofSearchCapacity,
    pub requested: u64,
    pub obligation: ObligationId,
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

pub(crate) fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}
