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
/// the module. The roster-round and work-pool variants are the
/// `proof_search_*` scalar-resource rows of the compiler-request
/// `Incomplete` outcome; the identity-space provision exceeds every
/// publishable scalar limit, so it travels by name alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofSearchCapacity {
    /// The scalar block-invariant roster pass's aggregate whole-module
    /// discovery and proof rounds (`proof_search_roster_rounds`).
    ScalarInvariantRosterRounds,
    /// The shared candidate-transport pool the roster pass's field-bound
    /// and join producers draw from (`proof_search_candidate_work`).
    ScalarInvariantCandidateWork,
    /// The separate transport pool of the cyclic-guarantee strengthening
    /// pass (`proof_search_cyclic_guarantee_work`).
    CyclicGuaranteeCandidateWork,
    /// The obligation-identity space the passes mint fresh arrival
    /// obligations from (`proof_obligation_identities`): the whole `u64`
    /// nonzero identity provision, larger than any scalar-resource limit
    /// the outcome table can publish.
    TerminalObligationIdentities,
}

impl ProofSearchCapacity {
    /// The scalar-resource name under which the compiler-request outcome
    /// table publishes this capacity's declines.
    pub const fn resource_name(self) -> &'static str {
        match self {
            Self::ScalarInvariantRosterRounds => "proof_search_roster_rounds",
            Self::ScalarInvariantCandidateWork => "proof_search_candidate_work",
            Self::CyclicGuaranteeCandidateWork => "proof_search_cyclic_guarantee_work",
            Self::TerminalObligationIdentities => "proof_obligation_identities",
        }
    }

    /// The provisioned bound the pass declined against; the producers keep
    /// the same numbers as their internal budgets.
    pub const fn limit(self) -> u64 {
        match self {
            Self::ScalarInvariantRosterRounds => 128,
            Self::ScalarInvariantCandidateWork => 4096,
            Self::CyclicGuaranteeCandidateWork => 4096,
            Self::TerminalObligationIdentities => u64::MAX,
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
    /// rejection that asserts nothing was left to find. `obligation` names
    /// the unproved site; the refusal still prevents `checked` publication
    /// exactly as any other lowering error does.
    ProofSearchCapacityExhausted {
        capacity: ProofSearchCapacity,
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

    /// The named capacity and unproved obligation when this refusal is a
    /// proof-search capacity exhaustion — the `Incomplete` outcome class a
    /// compiler-request publication maps — and absent when the refusal is an
    /// ordinary rejection.
    pub fn incomplete_capacity(&self) -> Option<(ProofSearchCapacity, ObligationId)> {
        match self {
            Self::ProofSearchCapacityExhausted {
                capacity,
                obligation,
            } => Some((*capacity, *obligation)),
            _ => None,
        }
    }
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
