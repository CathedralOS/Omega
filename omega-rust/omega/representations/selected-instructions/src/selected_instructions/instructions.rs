//! Executable selected forms and their explicit operand/effect interfaces.
use super::{SelectedInstructionId, SelectedInstructionProvenance, SelectedOperand};
use optimization_core::AcceptedObligationFactIdentity;
use register_model::{RegisterConstraintKey, RegisterUnitId};
use semantic_vocabulary::{IntegerValue, MachineId, ObligationId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub id: SelectedInstructionId,
    pub kind: SelectedInstructionKind,
    pub constraint: RegisterConstraintKey,
    pub operands: Vec<SelectedOperand>,
    pub implicit_uses: Vec<RegisterUnitId>,
    pub implicit_defs: Vec<RegisterUnitId>,
    pub clobbers: Vec<RegisterUnitId>,
    pub provenance: SelectedInstructionProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    /// Consume the low signed-i32 carrier and terminate through the selected host.
    HostedExitProcessI32,
    /// Closed hosted write(1, &low_byte(input), 1), continuing only after success.
    HostedReadByte {
        slot: crate::LocalStorageSlotId,
    },
    HostedWriteByteI32 {
        slot: super::LocalStorageSlotId,
    },
    /// Store the low exact-width bits through the original referent pointer.
    Store {
        byte_offset: u32,
        byte_size: u8,
    },
    /// Form a projected referent address without observing its contents.
    AddressOffset {
        byte_offset: u32,
    },
    /// Private 64-bit byte-view descriptor address: (backing + offset) modulo 2^64.
    /// Valid backing and subslice bounds restrict wrap to an exclusive-bound empty
    /// view, whose zero pointer is a valid empty carrier. No memory is accessed,
    /// authority created, or source Exact/Wrapping arithmetic claimed.
    ByteViewAddress,
    /// Load one byte from base plus byte index and zero-extend the register result.
    Load8Indexed,
    Load64 {
        byte_offset: u32,
    },
    /// Load exactly one byte and zero-extend its raw bits into GPR storage.
    /// The result retains its scalar type; this is not semantic integer widening.
    Load8 {
        byte_offset: u32,
    },
    /// Load exactly two bytes and zero-extend their raw bits into GPR storage.
    /// The result retains its scalar type; this is not semantic integer widening.
    Load16 {
        byte_offset: u32,
    },
    /// Load exactly four bytes and zero-extend their raw bits into GPR storage.
    Load32 {
        byte_offset: u32,
    },
    Store64 {
        slot: super::FrameStorageSlotId,
        byte_offset: u32,
    },
    FrameAddress {
        slot: super::FrameStorageSlotId,
        byte_offset: u32,
    },
    CallUnit {
        callee: MachineId,
    },
    CompareI64Zero,
    /// Compare two i64 register bit-patterns and define the target condition
    /// state consumed by conditional control. This instruction has no scalar
    /// result; equality is represented by the zero condition.
    CompareI64,
    MaterializeI64 {
        value: IntegerValue,
    },
    CopyI64,
    /// Preserve the IEEE binary32 payload while moving from an FP ABI home to GPR storage.
    Float32ToBits,
    /// Preserve the IEEE binary64 payload while moving from an FP ABI home to GPR storage.
    Float64ToBits,
    /// Restore the exact binary32 payload into an FP ABI home without arithmetic conversion.
    BitsToFloat32,
    /// Restore the exact binary64 payload into an FP ABI home without arithmetic conversion.
    BitsToFloat64,
    /// Zero-extend the low eight input bits into the complete result register.
    ZeroExtendU8,
    /// Normalize the low 32 input bits; upper ABI register bits are not meaningful.
    ZeroExtendU32,
    /// Exact mathematical addition whose source proof obligation was
    /// discharged before target lowering. A validated legalization theorem
    /// may transport a narrower exact operation to this i64 form; the selected
    /// receipt retains both the legal-plan and legalization-validator roots.
    ExactAddI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Exact mathematical subtraction whose Psi proof obligation was
    /// discharged before target lowering. Target constraints retain any
    /// architectural flag writes needed by its physical realization.
    ExactSubtractI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Exact mathematical addition with the right source value encoded as an
    /// instruction immediate. Proof and source-value custody remain explicit.
    ExactAddI64Immediate {
        immediate: IntegerValue,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Exact mathematical subtraction with the right source value encoded as
    /// an instruction immediate. Operand order, proof, and source-value
    /// custody remain explicit.
    ExactSubtractI64Immediate {
        immediate: IntegerValue,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    ConditionalBranchNonZero,
    /// Branch on the unsigned-U64 less-than predicate established by the
    /// immediately preceding comparison. The comparison remains a distinct
    /// selected instruction; this zero-operand terminator names the exact
    /// predicate consumed from target condition state.
    ConditionalBranchU64LessThan,
    /// Branch on signed-I64 strict less-than from the immediately preceding
    /// comparison.
    ConditionalBranchI64LessThan,
    Jump,
    ReturnI64,
    /// Value-less semantic return. This is deliberately distinct from
    /// `ReturnI64` even on targets where both select the same opcode.
    ReturnUnit,
    /// Direct internal call with two fixed U64 inputs and one fixed U64
    /// result. The target constraint row owns the exact ABI views and complete
    /// call clobbers; `callee` retains relocation/publication custody.
    CallI64 {
        callee: MachineId,
    },
}
