//! Lowering failure vocabulary shared by every producer in this crate.
//!
//! Unsupported constructs fail closed with a static reason; structured
//! variants retain the identity a caller needs to report or test the refusal.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use language_semantics::PermissionClaimIdentity;
use semantic_vocabulary::{
    ContentProjectionIdentity, PlaceId, PropositionError, StructuralPlaceKind,
};

use crate::proofs::float_meaning_projection::FloatMeaningProjectionLoweringError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    MachineNotFound(String),
    AmbiguousMachineName(String),
    /// A closure names storage absent from its retained type catalog.
    MissingUnitStructuralType(String),
    MissingUnitStructuralDomainCarrier {
        domain: language_semantics::SemanticDomainId,
        carrier: semantic_vocabulary::StructuralTypeId,
    },
    DebugSourceFileCountOverflow,
    DebugSourceLengthOverflow,
    MissingDebugSourceFile(usize),
    DebugSemanticCodec(terminal_codec::CodecError),
    InvalidDebugMap(terminal_codec::DebugMapError),
    InvalidControlGraph(terminal_semantics::control_graph::ControlGraphError),
    InvalidServiceParameterShape(checked_trees::CheckedServiceParameterError),
    InvalidServiceIdentity(String),
    InvalidFloatMeaningProjection(FloatMeaningProjectionLoweringError),
    InvalidQuotientCorrespondence(Vec<String>),
    InvalidUnitMachinePlan {
        machine: String,
        reason: &'static str,
        /// The checked gap of the named machine (or of the body its call
        /// chain reached first without one): the unsupported construct at its
        /// source span, when the checked record names one.
        gap: Option<Diagnostic>,
    },
    Unsupported(&'static str),
    InvalidSealedAccessorDeclaration(Diagnostic),
    /// A second transfer or disposal overlaps storage already removed from
    /// this occurrence. Keep both paths so the failing ownership join is visible.
    OverlappingProjectedCustody {
        reason: &'static str,
        place: PlaceId,
        earlier: Vec<terminal_psi::StructuralPathSegment>,
        requested: Vec<terminal_psi::StructuralPathSegment>,
    },
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
    /// The refusal for a body this compiler cannot lower yet, reported at the
    /// unsupported construct's source span rather than as this lowering error.
    pub fn plan_gap(&self) -> Option<&Diagnostic> {
        match self {
            Self::InvalidUnitMachinePlan { gap: Some(gap), .. } => Some(gap),
            _ => None,
        }
    }
}

diagnostics::debug_display!(LoweringError);

impl std::error::Error for LoweringError {}

pub(crate) fn unsupported<T>(message: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::Unsupported(message))
}

/// Translate an unsupported-plan lookup failure into the machine-plan error,
/// attaching the checked flow gap for `symbol`.
pub(crate) fn unit_machine_plan_error<'a>(
    checked: &'a CheckedTrees,
    machine_name: &'a str,
    symbol: symbols::SymbolHandle,
) -> impl FnOnce(LoweringError) -> LoweringError + 'a {
    move |error| match error {
        LoweringError::Unsupported(reason) => LoweringError::InvalidUnitMachinePlan {
            machine: machine_name.to_owned(),
            reason,
            gap: checked
                .facts
                .flow
                .terminal_unit_effects
                .gap_diagnostic(symbol, &checked.symbols),
        },
        error => error,
    }
}
