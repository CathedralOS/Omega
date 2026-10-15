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
    ZeroInitializedArgumentInvalid {
        argument_index: u32,
    },
    /// A raw execution cannot acquire exit authority from its effect handler.
    UnboundExternalCompletions {
        missing: Vec<crate::capabilities::TerminalExternalCompletionDemand>,
    },
    ExternalCompletion(crate::results::TerminalExternalCompletion),
    ScalarEntryQualificationUnsupported,
    UnsupportedCapability(crate::capabilities::TerminalUnsupportedCapability),
    /// A well-formed semantic case this implementation cannot yet execute.
    /// Invalid operation contracts must retain their malformed-error variants.
    UnsupportedSemanticVariant(&'static str),
    /// The artifact's exact affine cleanup transaction does not match the
    /// interpreter's live ownership paths.
    AffineFrontierMismatch,
    /// A projection cannot be represented exactly by the interpreter's current
    /// path-aware structural model, so execution fails closed.
    AffineProjectionNotRepresentable,
    /// A direct-entry placed-view input was not established by any supplied
    /// `TerminalPlacedViewEstablishment`, or an ordinary actual names no live
    /// compatible placed occurrence. Neither type nor pointer identity alone
    /// grants custody for a declared input.
    PlacedViewInputsRequireCustody,
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
    CompletionReceiptMismatch,
    ProviderInstallationIdentityMismatch,
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

impl From<FuelMeterError> for TerminalInterpretError {
    fn from(error: FuelMeterError) -> Self {
        Self::Fuel(error)
    }
}

impl std::fmt::Display for TerminalInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalInterpretError {}
