use crate::shared::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    SemanticIdentity(CodecError),
    /// Terminal preserves the exact payloadless sum case, but Omega has no
    /// target-neutral abstract operation for realizing that structural value.
    UnsupportedPayloadlessCase(psi_core::OperationId),
    /// Psi preserves exact byte-sequence literals, but native realization is
    /// deliberately fenced until the selected boundary has a byte-view ABI.
    UnsupportedByteSequenceLiteral(psi_core::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin one exact
    /// descriptor, its initializer/latest selections, and its indirect row.
    InvalidDynamicCall(psi_core::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin the exact
    /// whole-root write-only parameter and its preceding scalar definition.
    InvalidWriteOnlyPrimitiveStore(psi_core::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin the exact
    /// parameter root, structural path and field, or typed scalar value.
    InvalidStructuralScalarFieldStore(psi_core::OperationId),
    /// Independent Terminal-to-Omega projection could not rejoin the exact
    /// shared parameter root, integer field, and typed scalar result.
    InvalidIntegerStructuralField(psi_core::OperationId),
    /// Terminal retains scalar inputs to a Unit-returning internal call, but
    /// target-neutral Omega has not yet admitted that call carrier.
    UnsupportedUnitCallScalarArguments(psi_core::OperationId),
    ScalarReturnFromUnitMachine(MachineId),
    UnitReturnFromScalarMachine(MachineId),
    /// The verified structural-result machine is wider than the exact
    /// singleton whole-root passthrough currently carried into Omega.
    UnsupportedStructuralResult(MachineId),
    /// A structural return appeared on a non-structural-result machine.
    UnsupportedStructuralReturn {
        machine: MachineId,
        edge: psi_core::EdgeId,
    },
    VerifiedEntryMachineMissing(MachineId),
    VerifiedBlockMissing {
        machine: MachineId,
        block: BlockId,
    },
    VerifiedJumpArityMismatch {
        edge: psi_core::EdgeId,
    },
    VerifiedWrappingAddMalformed(psi_core::OperationId),
    VerifiedSaturatingAddMalformed(psi_core::OperationId),
    VerifiedWrappingSubtractMalformed(psi_core::OperationId),
    VerifiedSaturatingSubtractMalformed(psi_core::OperationId),
    VerifiedWrappingMultiplyMalformed(psi_core::OperationId),
    VerifiedExactDivideMalformed(psi_core::OperationId),
    VerifiedExactRemainderMalformed(psi_core::OperationId),
    VerifiedWrappingDivideMalformed(psi_core::OperationId),
    VerifiedWrappingRemainderMalformed(psi_core::OperationId),
    VerifiedSaturatingDivideMalformed(psi_core::OperationId),
    VerifiedSaturatingRemainderMalformed(psi_core::OperationId),
    VerifiedSaturatingMultiplyMalformed(psi_core::OperationId),
    VerifiedIntegerBitwiseMalformed(psi_core::OperationId),
    VerifiedIntegerWidenMalformed(psi_core::OperationId),
    VerifiedIntegerExactCastMalformed(psi_core::OperationId),
    VerifiedWrappingShiftMalformed(psi_core::OperationId),
    VerifiedExactShiftMalformed(psi_core::OperationId),
    VerifiedIeeeFloatMalformed(psi_core::OperationId),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}
