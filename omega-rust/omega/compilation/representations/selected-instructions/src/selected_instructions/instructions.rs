//! Executable selected forms and their explicit operand/effect interfaces.
use super::SaturatingCarrier;
use super::{SelectedInstructionId, SelectedInstructionProvenance, SelectedOperand};
use optimization_core::AcceptedObligationFactIdentity;
use register_model::{RegisterConstraintKey, RegisterUnitId};
use semantic_vocabulary::{IntegerValue, MachineId, ObligationId, OperationId, PlaceId};

use super::PackedByteWidth;
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

/// The roster position of the instruction keyed `id`, if present.
pub fn instruction_position(
    instructions: &[SelectedInstruction],
    id: SelectedInstructionId,
) -> Option<usize> {
    instructions
        .iter()
        .position(|instruction| instruction.id == id)
}

impl SelectedInstruction {
    /// The operand record numbered `operand`, when declared.
    pub fn operand(&self, operand: u16) -> Option<&SelectedOperand> {
        self.operands
            .iter()
            .find(|candidate| candidate.operand == operand)
    }
}

/// Call control keeps implementation selection apart from the data ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedCallTarget {
    Machine(MachineId),
    Dynamic { interface: u32, slot: u32 },
}

impl SelectedCallTarget {
    pub const fn from_native(target: target_operations::NativeCallTarget) -> Self {
        match target {
            target_operations::NativeCallTarget::Machine(machine) => Self::Machine(machine),
            target_operations::NativeCallTarget::Dynamic {
                interface, slot, ..
            } => Self::Dynamic { interface, slot },
        }
    }

    pub const fn machine(self) -> Option<MachineId> {
        match self {
            Self::Machine(machine) => Some(machine),
            Self::Dynamic { .. } => None,
        }
    }

    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::Machine(machine) => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, machine.get().to_le_bytes());
            }
            Self::Dynamic { interface, slot } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, interface.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, slot.to_le_bytes());
            }
        }
    }
}

/// Immutable image storage is selected by identity, independent of its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedImageAddressReference {
    ByteLiteral {
        operation: OperationId,
        place: PlaceId,
    },
    ImplementationTable {
        table: u32,
    },
}

impl SelectedImageAddressReference {
    pub fn encode_identity(self, bytes: &mut Vec<u8>) {
        match self {
            Self::ByteLiteral { operation, place } => {
                bytes.push(0);
                crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            }
            Self::ImplementationTable { table } => {
                bytes.push(1);
                crate::codec_bytes::push_encoded(bytes, table.to_le_bytes());
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    /// One sealed external event with exact-width transfer and retained contract.
    PlacedScalarAccess {
        form: super::PlacedScalarForm,
        contract: [u8; 32],
        byte_offset: u32,
    },
    /// Full coherent-memory barrier, with no address or value operands.
    AtomicFence,
    /// An indivisible sequentially consistent access. Address is operand 0;
    /// value inputs follow it, then the observed prior (absent for Store).
    /// Input registers are preserved; the observed output is early-clobber.
    AtomicAccess {
        form: super::AtomicForm,
    },
    /// One nearest-even IEEE operation on raw payloads in general registers:
    /// operands are left, right, then the defined result. Scratch floating
    /// registers and the sticky status flags are declared clobbers.
    NearestIeeeFloatArithmetic {
        form: super::IeeeFloatArithmeticForm,
    },
    /// Post-call adapter: raw result, or left/right/raw result, then output.
    FloatResultPolicy {
        form: super::FloatResultPolicyForm,
    },
    /// Unconditional trap realizing the exact semantic Crash terminator.
    Crash,
    /// Copy exactly count bytes between disjoint valid ranges. Inputs 0/1/2
    /// are preserved source/destination/count; 3/4 are early-clobber scratch.
    /// Count zero accesses neither pointer. Flags are clobbered.
    CopyBytes,
    /// Write zero to exactly count bytes at the destination. Inputs 0/1 are
    /// preserved destination/count; 2 is the early-clobber cursor scratch.
    /// Count zero accesses no memory. Flags are clobbered.
    ZeroBytes,
    /// Read exactly the codec-selected 1..8 bytes from the original backing.
    /// Operands are preserved address/codec and early-clobber cursor/count/byte/result.
    /// Codec bit 8 selects sign extension; float and Boolean payloads retain raw bits.
    /// Flags are clobbered. The admitted codec is an immutable runtime-ABI value.
    LoadEncodedScalar,
    /// Write exactly the codec-selected 1..8 low bytes, little-endian, in place.
    /// Operands are preserved address/codec/value and early-clobber cursor/count/payload.
    /// Fit and write authority are admitted before this operation. Flags are clobbered.
    /// The codec has the same immutable admitted-domain contract as LoadEncodedScalar;
    /// these operations do not admit arbitrary runtime words as storage metadata.
    StoreEncodedScalar,
    /// Evaluate `predicate` over `count` bytes beginning at the input base.
    /// Inputs 0/1 are preserved base/count; 2/3/4 are early-clobber because
    /// the prologue seeds all three before the loop reads the inputs;
    /// operand 4 is the defined 0/1 result. Count zero reads no bytes.
    /// Flags are clobbered.
    ScanByteSequence {
        predicate: terminal_psi::ByteSequencePredicate,
    },
    /// Compare `left_count` bytes at the left base with `right_count` bytes
    /// at the right base. Inputs 0-3 are preserved
    /// left base/left count/right base/right count; 4-6 are early-clobber
    /// scratch (cursor and both loaded bytes); 7 is the defined 0/1 result.
    /// Mismatched counts produce 0 and read no bytes; equal zero counts
    /// produce 1 likewise. Flags are clobbered.
    CompareByteSequences,
    LoadPacked {
        byte_offset: u32,
        width: PackedByteWidth,
    },
    StorePacked {
        byte_offset: u32,
        width: PackedByteWidth,
    },
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
    /// Address of exact immutable image storage. Its owner's relocation
    /// binds the same PC-relative address form at object construction.
    ImmutableImageAddress {
        reference: SelectedImageAddressReference,
    },
    CallUnit {
        target: SelectedCallTarget,
    },
    CompareI64Zero,
    /// Compare two i64 register bit-patterns and define the target condition
    /// state consumed by conditional control. This instruction has no scalar
    /// result; equality is represented by the zero condition.
    CompareI64,
    /// Compare one i64 register bit-pattern against the encoded unsigned
    /// immediate and define the same target condition state as `CompareI64`.
    /// This instruction has no scalar result; the immediate retains the exact
    /// literal source value admitted by the target encoding.
    CompareI64Immediate {
        immediate: IntegerValue,
    },
    MaterializeI64 {
        value: IntegerValue,
    },
    /// Unsigned division admitted by this operation's definedness proof.
    ExactDivideU64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Unsigned remainder admitted by this operation's definedness proof.
    /// The realization shares the unsigned divide hardware and selects the
    /// high-half remainder register instead of the quotient.
    ExactRemainderU64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Signed remainder of normalized i64 operands with a proven nonzero divisor.
    /// The wrapping overflow case i64::MIN % -1 produces zero, not a trap.
    WrappingRemainderI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Signed i64 division with a proven nonzero divisor. The wrapping
    /// overflow case i64::MIN / -1 produces i64::MIN, not a trap. Narrow
    /// normalized carriers share it: their i64 quotient cannot fault, and a
    /// narrow signed MIN / -1 widens to -MIN, which the subsequent carrier
    /// normalization truncates back to the wrapped MIN.
    WrappingDivideI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Signed division of normalized i64 operands admitted by this
    /// operation's definedness proof: the divisor is provably nonzero and the
    /// quotient provably representable in the declared carrier, so IDIV's
    /// overflow pair cannot occur. Every non-u64 fixed carrier selects this
    /// entry because scalar transport normalizes narrow operands into i64
    /// registers whose signed division is already canonical.
    ExactDivideI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Signed remainder of normalized i64 operands admitted by this
    /// operation's definedness proof. The realization shares the signed
    /// divide hardware and selects the high-half remainder register instead
    /// of the quotient.
    ExactRemainderI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Addition of normalized carriers clamped to the carrier bounds; no
    /// Exact proof is implied. The u64 carrier is the three-operand
    /// carry-select form; every other carrier adds in 64 bits (or detects the
    /// i64 overflow flag) and clamps through an early-clobber bound scratch in
    /// operand 3, so the result stays normalized for later source uses.
    SaturatingAdd {
        carrier: SaturatingCarrier,
    },
    /// Subtraction clamped to the carrier bounds. Unsigned carriers use the
    /// three-operand borrow-select form (a zero-normalized narrow difference
    /// borrows exactly when the u64 one does); signed carriers clamp through
    /// the operand-3 scratch like `SaturatingAdd`.
    SaturatingSubtract {
        carrier: SaturatingCarrier,
    },
    /// Division with a proven nonzero divisor, clamped to the carrier bounds.
    /// Unsigned carriers never overflow and share the unsigned divide form;
    /// signed narrow carriers clamp the one out-of-range quotient MIN / -1 to
    /// MAX; the i64 carrier must detect that case before dividing because the
    /// hardware quotient wraps (AArch64) or faults (x86-64).
    SaturatingDivide {
        carrier: SaturatingCarrier,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Remainder with a proven nonzero divisor under the named carrier. The
    /// mathematical remainder always lies inside the carrier, so no clamp is
    /// needed: signed carriers share the signed remainder form (whose one
    /// wrapped case MIN % -1 already yields the mathematical zero) and
    /// unsigned carriers share the exact unsigned remainder form.
    SaturatingRemainder {
        carrier: SaturatingCarrier,
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Multiplication clamped to the carrier bounds; no obligation, since
    /// saturation defines every product. Operands are `[left, right, result,
    /// scratch]` for every carrier. Narrow carriers multiply their normalized
    /// operands exactly in 64 bits and clamp through the scratch (an unsigned
    /// compare for unsigned carriers, whose u32 product may exceed i64::MAX).
    /// The 64-bit carriers detect overflow from the product's high half: the
    /// i64 carrier selects MAX or MIN by the product's sign, and the u64
    /// carrier selects MAX. The x86-64 u64 form is `MUL` on the fixed RAX:RDX
    /// pair, with the divisor-style RCX pin keeping the right operand out of
    /// RDX.
    SaturatingMultiply {
        carrier: SaturatingCarrier,
    },
    /// One `Trapping` primitive at the form's carrier: the exact result, or
    /// an inline architectural trap (the `Crash` leaf's bytes) when the
    /// settled Trapping predicate holds. The check is the policy, so no
    /// obligation is carried and the instruction is never dead: its trap is
    /// observable even when the result is unused. Operands are `[left,
    /// right, result, scratch]` (a shift's right operand is its count) and
    /// `[operand, result, scratch]` for a conversion; the result and scratch
    /// are early-clobber, and the result is normalized for later source uses.
    TrappingInteger {
        form: super::TrappingForm,
    },
    /// Add register payloads modulo 2^64 without an Exact overflow obligation.
    /// Narrow semantic results require a subsequent signed/unsigned normalization.
    WrappingAddI64,
    /// Subtract register payloads modulo 2^64; narrow results normalize after.
    WrappingSubtractI64,
    /// Multiply register payloads modulo 2^64; narrow results normalize after.
    WrappingMultiplyI64,
    CopyI64,
    /// Bitwise intersection of normalized integer carriers; no arithmetic overflow.
    BitwiseAndI64,
    /// Bitwise union of normalized integer carriers; no arithmetic overflow.
    BitwiseOrI64,
    BitwiseXorI64,
    /// Bitwise complement of one normalized integer carrier. Narrow unsigned
    /// results require a subsequent normalization.
    BitwiseNotI64,
    /// Left shift of a normalized integer carrier by a count already reduced
    /// modulo the value width upstream. The count lives in the second source
    /// register (x86-64 realizes it in CL); narrow semantic results require a
    /// subsequent normalization.
    WrappingShiftLeftI64,
    /// Arithmetic (sign-filling) right shift of a normalized signed carrier by
    /// a reduced count.
    WrappingShiftRightI64,
    /// Logical (zero-filling) right shift of a normalized unsigned carrier by
    /// a reduced count.
    WrappingShiftRightU64,
    /// Left shift by an independently typed count proven inside `[0, width)`.
    /// The representable-result obligation stays attached; the realization is
    /// the same shift form with the proven count used directly.
    ExactShiftLeftI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Arithmetic (sign-filling) right shift by a proven in-range count; the
    /// in-range obligation stays attached.
    ExactShiftRightI64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
    /// Logical (zero-filling) right shift by a proven in-range count; the
    /// in-range obligation stays attached.
    ExactShiftRightU64 {
        obligation: ObligationId,
        accepted_fact: AcceptedObligationFactIdentity,
    },
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
    /// Zero-extend the low 16 input bits into the complete result register.
    ZeroExtendU16,
    /// Sign-extend the low eight input bits into the complete result register.
    SignExtendI8,
    /// Sign-extend the low 16 input bits into the complete result register.
    SignExtendI16,
    /// Sign-extend the low 32 input bits into the complete result register.
    SignExtendI32,
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
    /// Exact mathematical multiplication whose source proof obligation was
    /// discharged before target lowering. A validated legalization theorem
    /// may transport a narrower exact operation to this i64 form; the
    /// retained representability proof makes the i64 result canonical.
    ExactMultiplyI64 {
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
    /// Return one scalar through the exact ABI register constraint.
    ReturnScalar,
    /// Return the complete direct aggregate ABI register-fragment roster.
    ReturnAggregate {
        fragment_count: u8,
    },
    /// Value-less semantic return. This is deliberately distinct from
    /// `ReturnScalar` even on targets where both select the same opcode.
    ReturnUnit,
    /// Internal scalar call with register-passed inputs and one result.
    /// The requirement's ABI row owns data views and complete clobbers. An
    /// indirect target adds a separate selector input before result definitions;
    /// it never occupies an ordinary parameter placement.
    CallScalar {
        target: SelectedCallTarget,
    },
    /// Direct evaluated normalized foreign call. The constraint row owns the
    /// exact ABI views and complete call clobbers; `boundary` retains
    /// publication custody while the function's foreign-call roster carries
    /// the evaluated locator, entry plan, and provider execution.
    NormalizedForeignCall {
        boundary: semantic_vocabulary::BoundaryMachineId,
        ordinal: u32,
    },
    /// Ordinary call defining every direct aggregate result fragment.
    CallAggregate {
        target: SelectedCallTarget,
    },
    /// Consume target condition state and define a complete GPR containing zero or one.
    MaterializeBooleanEqual,
    MaterializeBooleanU64LessThan,
    MaterializeBooleanI64LessThan,
    MaterializeBooleanU64LessOrEqual,
    MaterializeBooleanI64LessOrEqual,
    /// Preserve ambient floating controls in private caller-frame storage.
    SaveFloatingControl {
        slot: crate::LocalStorageSlotId,
    },
    /// Restore controls before interpreting a returning foreign result.
    RestoreFloatingControl {
        slot: crate::LocalStorageSlotId,
    },
}

impl SelectedInstructionKind {
    pub const fn call_target(self) -> Option<SelectedCallTarget> {
        match self {
            Self::CallScalar { target }
            | Self::CallAggregate { target }
            | Self::CallUnit { target } => Some(target),
            _ => None,
        }
    }

    pub const fn machine_callee(self) -> Option<MachineId> {
        match self {
            Self::CallScalar {
                target: SelectedCallTarget::Machine(callee),
            }
            | Self::CallAggregate {
                target: SelectedCallTarget::Machine(callee),
            }
            | Self::CallUnit {
                target: SelectedCallTarget::Machine(callee),
            } => Some(callee),
            _ => None,
        }
    }
}

/// The layout-dependent control transfers: conditional branches and jumps
/// resolve through image layout rather than the scalar-register selected-form
/// path. ISA validators, byte constructors and decode checks reject this
/// roster uniformly, so it is spelled once here instead of per target.
#[macro_export]
macro_rules! layout_dependent_selected_transfers {
    () => {
        $crate::SelectedInstructionKind::ConditionalBranchNonZero
            | $crate::SelectedInstructionKind::ConditionalBranchU64LessThan
            | $crate::SelectedInstructionKind::ConditionalBranchI64LessThan
            | $crate::SelectedInstructionKind::Jump
    };
}

/// Kinds realized by dedicated encoders — float bit transfers, memory and
/// byte-sequence forms, hosted services, floating control, calls and
/// materialization — dispatched before the scalar-register path. The scalar
/// path's validation, byte construction and decode checks reject this roster
/// uniformly, so it is spelled once here instead of per target.
#[macro_export]
macro_rules! dedicated_selected_forms {
    () => {
        $crate::SelectedInstructionKind::PlacedScalarAccess { .. }
            | $crate::SelectedInstructionKind::Float32ToBits
            | $crate::SelectedInstructionKind::Float64ToBits
            | $crate::SelectedInstructionKind::BitsToFloat32
            | $crate::SelectedInstructionKind::BitsToFloat64
            | $crate::SelectedInstructionKind::CopyBytes
            | $crate::SelectedInstructionKind::ZeroBytes
            | $crate::SelectedInstructionKind::ScanByteSequence { .. }
            | $crate::SelectedInstructionKind::LoadEncodedScalar
            | $crate::SelectedInstructionKind::StoreEncodedScalar
            | $crate::SelectedInstructionKind::CompareByteSequences
            | $crate::SelectedInstructionKind::Load8Indexed
            | $crate::SelectedInstructionKind::Load8 { .. }
            | $crate::SelectedInstructionKind::Load16 { .. }
            | $crate::SelectedInstructionKind::Load32 { .. }
            | $crate::SelectedInstructionKind::Load64 { .. }
            | $crate::SelectedInstructionKind::LoadPacked { .. }
            | $crate::SelectedInstructionKind::StorePacked { .. }
            | $crate::SelectedInstructionKind::Store { .. }
            | $crate::SelectedInstructionKind::AddressOffset { .. }
            | $crate::SelectedInstructionKind::Store64 { .. }
            | $crate::SelectedInstructionKind::HostedExitProcessI32
            | $crate::SelectedInstructionKind::HostedReadByte { .. }
            | $crate::SelectedInstructionKind::SaveFloatingControl { .. }
            | $crate::SelectedInstructionKind::RestoreFloatingControl { .. }
            | $crate::SelectedInstructionKind::HostedWriteByteI32 { .. }
            | $crate::SelectedInstructionKind::FrameAddress { .. }
            | $crate::SelectedInstructionKind::CallUnit { .. }
            | $crate::SelectedInstructionKind::CallAggregate { .. }
            | $crate::SelectedInstructionKind::NormalizedForeignCall { .. }
            | $crate::SelectedInstructionKind::CallScalar { .. }
    };
}

#[macro_export]
macro_rules! exact_shift_i64_pair_pattern {
    () => {
        $crate::SelectedInstructionKind::ExactShiftLeftI64 { .. }
            | $crate::SelectedInstructionKind::ExactShiftRightI64 { .. }
    };
}

#[macro_export]
macro_rules! saturating_add_or_subtract_pattern {
    () => {
        $crate::SelectedInstructionKind::SaturatingAdd { .. }
            | $crate::SelectedInstructionKind::SaturatingSubtract { .. }
    };
}

#[macro_export]
macro_rules! load_width_triple_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::Load8 { $($fields)* }
            | $crate::SelectedInstructionKind::Load16 { $($fields)* }
            | $crate::SelectedInstructionKind::Load32 { $($fields)* }
    };
}

#[macro_export]
macro_rules! selected_call_triple_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::CallAggregate { $($fields)* }
            | $crate::SelectedInstructionKind::CallScalar { $($fields)* }
            | $crate::SelectedInstructionKind::CallUnit { $($fields)* }
    };
}

#[macro_export]
macro_rules! hosted_slot_quad_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::HostedReadByte { $($fields)* }
            | $crate::SelectedInstructionKind::HostedWriteByteI32 { $($fields)* }
            | $crate::SelectedInstructionKind::RestoreFloatingControl { $($fields)* }
            | $crate::SelectedInstructionKind::SaveFloatingControl { $($fields)* }
    };
}

#[macro_export]
macro_rules! exact_add_or_sub_pair_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::ExactAddI64Immediate { $($fields)* }
            | $crate::SelectedInstructionKind::ExactSubtractI64Immediate { $($fields)* }
    };
}

#[macro_export]
macro_rules! exact_wrapping_divrem_set_pattern {
    ($($fields:tt)*) => {
        $crate::SelectedInstructionKind::ExactDivideI64 { $($fields)* }
            | $crate::SelectedInstructionKind::ExactRemainderI64 { $($fields)* }
            | $crate::SelectedInstructionKind::ExactRemainderU64 { $($fields)* }
            | $crate::SelectedInstructionKind::WrappingDivideI64 { $($fields)* }
            | $crate::SelectedInstructionKind::WrappingRemainderI64 { $($fields)* }
    };
}
