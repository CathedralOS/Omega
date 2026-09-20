//! Lowering failure vocabulary shared by every producer in this crate.
//!
//! Unsupported constructs fail closed with a static reason; structured
//! variants retain the identity a caller needs to report or test the refusal.

use language_semantics::PermissionClaimIdentity;
use semantic_vocabulary::{
    ContentProjectionIdentity, ObligationId, PlaceId, PropositionError, StructuralPlaceKind,
};

use crate::proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;

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
    InvalidFloatMeaningProjection(FloatMeaningProjectionLoweringError),
    InvalidQuotientCorrespondence(Vec<String>),
    OperationProofUnavailable(ObligationId),
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

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

pub(crate) fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}
