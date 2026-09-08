use crate::shared::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    SemanticIdentity(CodecError),
    /// Abstract block entries cannot retain borrowed-view descriptor bindings yet.
    UnsupportedStructuralBlockParameters {
        machine: MachineId,
        block: BlockId,
    },
    /// Abstract successors currently retain only scalar bindings.
    UnsupportedStructuralSuccessorArguments {
        machine: MachineId,
        edge: semantic_vocabulary::EdgeId,
    },
    /// Terminal preserves the exact payloadless sum case, but Omega has no
    /// target-neutral abstract operation for realizing that structural value.
    UnsupportedPayloadlessCase(semantic_vocabulary::OperationId),
    /// Psi preserves exact byte-sequence literals, but native realization is
    /// deliberately fenced until the selected boundary has a byte-view ABI.
    UnsupportedByteSequenceLiteral(semantic_vocabulary::OperationId),
    /// Bounded byte-field replacement has no native storage realization yet.
    UnsupportedStructuralByteSequenceFieldStore(semantic_vocabulary::OperationId),
    /// Bounded byte-field metadata has no native storage realization yet.
    UnsupportedStructuralByteSequenceFieldLength(semantic_vocabulary::OperationId),
    /// Indexed bounded byte-field writes have no native storage realization yet.
    UnsupportedStructuralByteSequenceFieldByteStore(semantic_vocabulary::OperationId),
    /// The length observation did not retain its exact scalar u64 result.
    InvalidByteSequenceLength(semantic_vocabulary::OperationId),
    InvalidByteSequenceRead(semantic_vocabulary::OperationId),
    InvalidByteSequenceSubslice(semantic_vocabulary::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin one exact
    /// descriptor, its initializer/latest selections, and its indirect row.
    InvalidDynamicCall(semantic_vocabulary::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin the exact
    /// whole-root write-only parameter and its preceding scalar definition.
    InvalidWriteOnlyPrimitiveStore(semantic_vocabulary::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin the exact
    /// parameter root, structural path and field, or typed scalar value.
    InvalidStructuralScalarFieldStore(semantic_vocabulary::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin the exact
    /// shared parameter root, integer field, and typed scalar result.
    InvalidIntegerStructuralField(semantic_vocabulary::OperationId),
    /// The dynamic-call representation has no combined scalar-and-descriptor
    /// argument carrier yet. Ordinary Unit calls retain scalar arguments.
    UnsupportedUnitCallScalarAndDynamicArguments(semantic_vocabulary::OperationId),
    ScalarReturnFromUnitMachine(MachineId),
    UnitReturnFromScalarMachine(MachineId),
    /// The verified structural-result machine is wider than the exact
    /// singleton whole-root passthrough currently carried into Omega.
    UnsupportedStructuralResult(MachineId),
    /// A structural return appeared on a non-structural-result machine.
    UnsupportedStructuralReturn {
        machine: MachineId,
        edge: semantic_vocabulary::EdgeId,
    },
    VerifiedEntryMachineMissing(MachineId),
    VerifiedBlockMissing {
        machine: MachineId,
        block: BlockId,
    },
    VerifiedJumpArityMismatch {
        edge: semantic_vocabulary::EdgeId,
    },
    VerifiedWrappingAddMalformed(semantic_vocabulary::OperationId),
    VerifiedSaturatingAddMalformed(semantic_vocabulary::OperationId),
    VerifiedWrappingSubtractMalformed(semantic_vocabulary::OperationId),
    VerifiedSaturatingSubtractMalformed(semantic_vocabulary::OperationId),
    VerifiedWrappingMultiplyMalformed(semantic_vocabulary::OperationId),
    VerifiedExactDivideMalformed(semantic_vocabulary::OperationId),
    VerifiedExactRemainderMalformed(semantic_vocabulary::OperationId),
    VerifiedWrappingDivideMalformed(semantic_vocabulary::OperationId),
    VerifiedWrappingRemainderMalformed(semantic_vocabulary::OperationId),
    VerifiedSaturatingDivideMalformed(semantic_vocabulary::OperationId),
    VerifiedSaturatingRemainderMalformed(semantic_vocabulary::OperationId),
    VerifiedSaturatingMultiplyMalformed(semantic_vocabulary::OperationId),
    VerifiedIntegerBitwiseMalformed(semantic_vocabulary::OperationId),
    VerifiedIntegerWidenMalformed(semantic_vocabulary::OperationId),
    VerifiedIntegerExactCastMalformed(semantic_vocabulary::OperationId),
    VerifiedWrappingShiftMalformed(semantic_vocabulary::OperationId),
    VerifiedExactShiftMalformed(semantic_vocabulary::OperationId),
    VerifiedIeeeFloatMalformed(semantic_vocabulary::OperationId),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
