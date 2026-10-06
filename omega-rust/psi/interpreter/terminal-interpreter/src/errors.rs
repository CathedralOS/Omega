//! Every way interpretation, provider installation or artifact decoding can
//! fail.

use crate::effects::TerminalEffectRejection;
use crate::results::TerminalCrash;
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType, StructuralDomainId,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_fuel::FuelMeterError;
use terminal_psi::CrashCause;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInterpretError {
    ScalarEntryQualificationUnsupported,
    DynamicDescriptorInputMissing {
        machine: MachineId,
        parameter_ordinal: u32,
    },
    UnsupportedCapability(crate::capabilities::TerminalUnsupportedCapability),
    UnsupportedSemanticVariant(&'static str),
    /// The artifact's exact affine cleanup transaction does not match the
    /// interpreter's live ownership paths.
    AffineFrontierMismatch,
    /// A projection cannot be represented exactly by the interpreter's current
    /// path-aware structural model, so execution fails closed.
    AffineProjectionNotRepresentable,
    /// A direct-entry placed-view input was not established by any supplied
    /// `TerminalPlacedViewEstablishment`, or the roster carries a row on a
    /// non-entry machine — an input bound by its call, which this boundary
    /// cannot yet route. Execution fails closed rather than starting the
    /// entry machine with a declared input unbound.
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
    /// One root-authorized assumed binding contract failed admission replay
    /// against the verified artifact.
    AssumedBindingRejected {
        required: BoundaryMachineId,
        supplied_requirement: BoundaryMachineId,
        reason: AssumedBindingRejection,
    },
}

/// Why one assumed binding contract relationship was rejected at
/// installation admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingRejection {
    /// The required side names no boundary machine in the verified artifact.
    UnknownRequirement,
    /// The supplied side names no boundary machine in the verified artifact.
    UnknownSuppliedRequirement,
    /// An ordinary checked provider selection already supplies the required
    /// requirement: competing explicit checked and assumed supplies for the
    /// same operation reject rather than silently preferring the assumption.
    CompetingCheckedSupply,
    /// Another assumed binding already claims the required requirement —
    /// every required member still needs exactly one supply.
    CompetingAssumption,
    /// Following the supplied side's links reaches no selected
    /// implementation: the supplied side must have one selected complete
    /// provider and executable realization.
    UnselectedSupply,
    /// The assumed link resolves through itself or a cycle of assumed
    /// links — cyclic binding aliases reject.
    CyclicSupply,
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
