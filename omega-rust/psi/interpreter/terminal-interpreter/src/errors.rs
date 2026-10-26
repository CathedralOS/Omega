//! Every way interpretation, provider installation or artifact decoding can
//! fail.

use crate::effects::TerminalEffectRejection;
use crate::results::TerminalCrash;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_fuel::FuelMeterError;
use terminal_psi::CrashCause;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInterpretError {
    FoldedLayoutRequirement {
        owner: terminal_psi::FoldedLayoutOwner,
        reason: terminal_semantics::LayoutObservationError,
    },
    LayoutObservation {
        operation: OperationId,
        reason: terminal_semantics::LayoutObservationError,
    },
    UnsuppliedErasedEntryInputs {
        machine: MachineId,
        reason: &'static str,
    },
    ZeroInitializedArgumentInvalid {
        argument_index: u32,
    },
    /// A raw execution cannot acquire exit authority from its effect handler.
    UnboundExternalCompletions {
        missing: Vec<crate::capabilities::TerminalExternalCompletionDemand>,
    },
    ExternalCompletion(crate::results::TerminalExternalCompletion),
    ExternalCompletionProviderReturned {
        boundary: BoundaryMachineId,
        edge: semantic_vocabulary::EdgeId,
    },
    ScalarEntryQualificationUnsupported,
    UnsupportedCapability(crate::capabilities::TerminalUnsupportedCapability),
    /// A well-formed semantic case this implementation cannot yet execute.
    /// Invalid operation contracts must retain their malformed-error variants.
    UnsupportedSemanticVariant(&'static str),
    /// An established original storage home failed; no detached local value
    /// may substitute for a faulted or retired resident.
    ResidentStorage(TerminalEffectRejection),
    /// The artifact's exact affine cleanup transaction does not match the
    /// interpreter's live ownership paths.
    AffineFrontierMismatch,
    /// A cleanup receiver still has captured loans. Retain the descriptor
    /// carriers so a dead home is distinguishable from a live outgoing loan.
    NominalCleanupHasLiveLoans {
        cleanup: terminal_psi::NominalAffineCleanup,
        backing_identity: u64,
        backing_path: Vec<terminal_psi::StructuralPathSegment>,
        loan_carriers: Vec<(u64, Vec<terminal_psi::StructuralPathSegment>)>,
        loan_carrier_homes: Vec<(PlaceId, crate::values::TerminalStructuralValue)>,
    },
    StructuralReturnAffineFrontierMismatch {
        source: PlaceId,
        expected: Vec<terminal_psi::StructuralAffineDiscard>,
        actual: Vec<terminal_psi::StructuralAffineDiscard>,
    },
    UnsettledReturnFrontier {
        affine: Vec<terminal_psi::StructuralAffineDiscard>,
        claims: Vec<semantic_vocabulary::ClaimId>,
    },
    /// A structural return must carry exactly its active claims and leave no
    /// other owned claim live. Retain both rosters so missing consumption is
    /// distinguishable from a malformed result binding at the execution site.
    StructuralReturnClaimsMismatch {
        source: PlaceId,
        expected: Vec<terminal_psi::StructuralResultClaimBinding>,
        actual: Vec<terminal_psi::StructuralResultClaimBinding>,
        unreturned: Vec<semantic_vocabulary::ClaimId>,
    },
    /// A projection cannot be represented exactly by the interpreter's current
    /// path-aware structural model, so execution fails closed.
    AffineProjectionNotRepresentable,
    /// A direct-entry placed-view input was not established by any supplied
    /// `TerminalPlacedViewEstablishment`, or an ordinary actual names no live
    /// compatible placed occurrence. Neither type nor pointer identity alone
    /// grants custody for a declared input.
    PlacedViewInputsRequireCustody,
    /// Static correspondence or live backing for a local placed interpretation.
    PlacedReference(String),
    /// Two supplied establishments name the same roster row.
    PlacedViewInputEstablishmentDuplicate {
        machine: MachineId,
        position: u32,
    },
    /// A supplied establishment names no declared entry roster row: custody
    /// the artifact never demanded cannot be bound at its input boundary.
    PlacedViewInputEstablishmentUnexpected {
        machine: MachineId,
        position: u32,
    },
    /// An established referent overlaps another placed-view establishment's
    /// referent or a structural argument under exclusive access.
    PlacedViewInputEstablishmentAliasing(u64),
    /// Runtime backing does not rejoin the verified artifact's declarations.
    PlacedViewReferent(terminal_semantics::PlacedViewReferentError),
    PlacedViewInput(terminal_semantics::PlacedViewInputError),
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    ArgumentType {
        value: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    ArgumentIntegerOutsideType {
        value: ValueId,
    },
    StructuralArgumentCount {
        expected: usize,
        actual: usize,
    },
    StructuralArgumentType {
        place: PlaceId,
        expected: StructuralTypeId,
        actual: StructuralTypeId,
    },
    StructuralQualificationsNonCanonical,
    StructuralArgumentPreparation {
        stage: &'static str,
        detail: String,
    },
    StructuralStorePreparation {
        stage: &'static str,
        detail: String,
    },
    StructuralQualificationMissing(PlaceId),
    StructuralArgumentAliasing(u64),
    StructuralPrimitiveValueCount {
        expected: usize,
        actual: usize,
    },
    StructuralPrimitiveValueInvalid {
        argument_index: u32,
    },
    StructuralPrimitiveValueType {
        argument_index: u32,
        expected: ScalarType,
        actual: ScalarType,
    },
    StructuralPrimitiveStorageMissing(PlaceId),
    StructuralIdentityExhausted,
    StructuralScalarFieldMissing {
        source: PlaceId,
        field: StructuralFieldId,
    },
    StructuralScalarFieldArgumentInvalid {
        argument_index: u32,
        field: StructuralFieldId,
    },
    StructuralBooleanFieldArgumentInvalid {
        argument_index: u32,
        field: StructuralFieldId,
    },
    StructuralBooleanFieldMissing {
        source: PlaceId,
        field: StructuralFieldId,
    },
    BoundaryQualificationMissing {
        boundary: BoundaryMachineId,
        argument_index: u32,
        domain: StructuralDomainId,
    },
    ClaimTransferMismatch,
    ContentTransformationMismatch(&'static str),
    ResourceProfile {
        boundary: BoundaryMachineId,
        diagnostic: access_plans::AccessPlanDiagnostic,
    },
    ValueImage(layout_plans::MaterializationDiagnostic),
    CompletionReceiptMismatch,
    ProviderInstallationIdentityMismatch,
    FoldedConstantClosure(&'static str),
    TargetMachineSelectionIdentityMismatch,
    TargetLayoutSelectionIdentityMismatch,
    TargetLayoutSelectionMissing(terminal_psi::LayoutApplicationId),
    TargetLayoutSelection(terminal_installation::TargetLayoutSelectionError),
    TargetMachineSelectionMissing(terminal_psi::TargetMachineCallable),
    TargetMachineSelection(terminal_installation::TargetMachineSelectionError),
    RuntimeSupplySelectionRejected {
        boundary: BoundaryMachineId,
        reason: &'static str,
    },
    ProviderInstallationMissing(BoundaryMachineId),
    VerifiedEntryMachineMissing,
    VerifiedCallTargetMissing(MachineId),
    VerifiedBoundaryMachineMissing(BoundaryMachineId),
    VerifiedBlockMissing,
    VerifiedOperationMalformed,
    ExecutionSite {
        machine: MachineId,
        block: BlockId,
        operation: Option<OperationId>,
        cause: Box<TerminalInterpretError>,
    },
    StructuralReplacement {
        operation: OperationId,
        phase: &'static str,
        cause: Box<TerminalInterpretError>,
    },
    VerifiedStructuralPlaceMissing(PlaceId),
    VerifiedValueMissing(ValueId),
    EffectRejected {
        operation: OperationId,
        rejection: TerminalEffectRejection,
    },
    BoundaryCrashNotPermitted {
        operation: OperationId,
        boundary: BoundaryMachineId,
        cause: CrashCause,
        reason: crate::effects::crash_outcome::BoundaryCrashOutcomeError,
    },
    Crash(TerminalCrash),
    Fuel(FuelMeterError),
    Storage(evaluation_accounting::EvaluationAccountingError),
    StorageAllocationFailed {
        requested_bytes: usize,
        reason: String,
    },
}

pub use terminal_installation::{AssumedBindingRejection, ProviderInstallationError};

#[derive(Debug)]
pub enum TerminalArtifactInterpretError {
    ArtifactDecode(terminal_codec::CanonicalTerminalArtifactError),
    SemanticDecode(terminal_codec::CodecError),
    Execution(TerminalInterpretError),
}

impl std::fmt::Display for TerminalArtifactInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactInterpretError {}

/// A verified artifact proved inconsistent at runtime — execution fails
/// closed rather than continuing on a malformed operation.
pub(crate) fn malformed() -> TerminalInterpretError {
    TerminalInterpretError::VerifiedOperationMalformed
}

impl TerminalInterpretError {
    /// Retain the admission stage without discarding the underlying refusal.
    pub(crate) fn in_argument_preparation(self, stage: &'static str) -> Self {
        let detail = match self {
            Self::StructuralArgumentPreparation { stage, detail } => format!("{stage}: {detail}"),
            other => format!("{other:?}"),
        };
        Self::StructuralArgumentPreparation { stage, detail }
    }
}

impl From<FuelMeterError> for TerminalInterpretError {
    fn from(error: FuelMeterError) -> Self {
        Self::Fuel(error)
    }
}

impl std::fmt::Display for TerminalInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionSite {
                machine,
                block,
                operation,
                cause,
            } => {
                write!(
                    formatter,
                    "{cause} in {machine:?}/{block:?} at {operation:?}"
                )
            }
            Self::StructuralReplacement {
                operation,
                phase,
                cause,
            } => {
                write!(formatter, "{cause} during {phase} at {operation:?}")
            }
            Self::EffectRejected {
                operation,
                rejection,
            } => {
                write!(formatter, "{} at {operation:?}", rejection.reason)
            }
            Self::Fuel(reason) => reason.fmt(formatter),
            Self::Storage(reason) => reason.fmt(formatter),
            other => write!(formatter, "{other:?}"),
        }
    }
}

impl std::error::Error for TerminalInterpretError {}
