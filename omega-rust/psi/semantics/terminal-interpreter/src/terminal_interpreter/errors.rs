//! Every way interpretation, provider installation or artifact decoding can
//! fail.

use crate::terminal_interpreter::{TerminalCrash, TerminalEffectRejection};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_fuel::FuelMeterError;
use terminal_psi::CrashCause;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInterpretError {
    ScalarEntryQualificationUnsupported,
    UnsupportedSemanticVariant(&'static str),
    /// The artifact's exact affine cleanup transaction does not match the
    /// interpreter's live ownership paths.
    AffineFrontierMismatch,
    /// A projection cannot be represented exactly by the interpreter's current
    /// path-aware structural model, so execution fails closed.
    AffineProjectionNotRepresentable,
    /// The module declares placed-view inputs. That roster is semantic
    /// custody, not storage: no scalar, structural, or byte-sequence input
    /// can supply the referent each row names, and interpretation has no
    /// establishment route that lends one. Execution therefore fails closed
    /// rather than starting the entry machine with a declared input unbound.
    PlacedViewInputsRequireCustody,
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
    ProviderInstallationMissing(BoundaryMachineId),
    VerifiedEntryMachineMissing,
    VerifiedCallTargetMissing(MachineId),
    VerifiedBoundaryMachineMissing(BoundaryMachineId),
    VerifiedBlockMissing,
    VerifiedOperationMalformed,
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
        reason: terminal_verifier::BoundaryCrashOutcomeError,
    },
    Crash(TerminalCrash),
    Fuel(FuelMeterError),
}

#[derive(Debug)]
pub enum ProviderInstallationError {
    SemanticDecode(terminal_codec::CodecError),
    ProofDecode(terminal_codec::ProofCodecError),
    Verification(terminal_verifier::VerificationError),
    UnknownOrDuplicateSelection {
        boundary: BoundaryMachineId,
        candidate: MachineId,
    },
}

#[derive(Debug)]
pub enum TerminalArtifactInterpretError {
    ArtifactDecode(terminal_codec::CanonicalTerminalArtifactError),
    SemanticDecode(terminal_codec::CodecError),
    ProofDecode(terminal_codec::ProofCodecError),
    Verification(terminal_verifier::VerificationError),
    Execution(TerminalInterpretError),
}

impl std::fmt::Display for TerminalArtifactInterpretError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactInterpretError {}

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
