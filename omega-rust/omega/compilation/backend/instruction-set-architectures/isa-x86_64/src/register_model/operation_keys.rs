//! The constraint keys of the ordinary x86-64 operations: loads, stores,
//! float bit casts, hosted calls, arithmetic, comparisons and branches, and
//! the closed inventory every catalog must carry, extended from the key owners
//! that mint each call and return family.

use super::float_scalar_calls::{x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys};
use super::indirect_results::x86_64_indirect_aggregate_call_keys;
use super::packed_memory::{X86_64_LOAD_PACKED, X86_64_STORE_PACKED};
use crate::register_model::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN, X86_64_MICROSOFT_RETURN_UNIT,
    X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
};
use register_model::RegisterConstraintKey;

pub const X86_64_CRASH: RegisterConstraintKey = RegisterConstraintKey::instruction(780);

/// Raw-result policies use integer predicates and volatile vector scratch.
pub const X86_64_FLOAT_RESULT_TRAPPING: RegisterConstraintKey =
    RegisterConstraintKey::instruction(789);
pub const X86_64_FLOAT_RESULT_SATURATING: RegisterConstraintKey =
    RegisterConstraintKey::instruction(790);

pub const X86_64_LOAD8: RegisterConstraintKey = RegisterConstraintKey::instruction(730);

pub const X86_64_LOAD16: RegisterConstraintKey = RegisterConstraintKey::instruction(731);

pub const X86_64_LOAD32: RegisterConstraintKey = RegisterConstraintKey::instruction(714);

pub const X86_64_FLOAT32_TO_BITS: RegisterConstraintKey = RegisterConstraintKey::instruction(710);

pub const X86_64_FLOAT64_TO_BITS: RegisterConstraintKey = RegisterConstraintKey::instruction(711);

pub const X86_64_BITS_TO_FLOAT32: RegisterConstraintKey = RegisterConstraintKey::instruction(712);

pub const X86_64_BITS_TO_FLOAT64: RegisterConstraintKey = RegisterConstraintKey::instruction(713);

/// Nearest-even IEEE arithmetic on general-register payloads: two inputs and
/// one result, with MXCSR read and scratch `xmm4`/`xmm5` clobbered.
/// Balanced private MXCSR storage, preserving RSP and all incoming operands.
pub const X86_64_IEEE_FLOAT_DIRECTED_BINARY: RegisterConstraintKey =
    RegisterConstraintKey::instruction(792);

/// Directed square root: one input, one result, MXCSR read, RSP for the
/// private control envelope, and scratch `xmm4`/`xmm5` clobbered.
pub const X86_64_IEEE_FLOAT_DIRECTED_UNARY: RegisterConstraintKey =
    RegisterConstraintKey::instruction(793);

pub const X86_64_IEEE_FLOAT_ARITHMETIC: RegisterConstraintKey =
    RegisterConstraintKey::instruction(785);

/// The same IEEE forms with one general-register input (negation, square
/// root and conversions) and one result.
pub const X86_64_IEEE_FLOAT_UNARY: RegisterConstraintKey = RegisterConstraintKey::instruction(786);

/// Unary IEEE integer conversions that additionally clobber RFLAGS while
/// retaining the unary form's MXCSR and scratch-vector effects.
pub const X86_64_IEEE_FLOAT_CONVERSION_CONTROL: RegisterConstraintKey =
    RegisterConstraintKey::instruction(788);

/// Three general-register inputs and one result, with the same floating
/// controls and scratch-register effects as the unary and binary forms.
pub const X86_64_IEEE_FLOAT_TERNARY: RegisterConstraintKey =
    RegisterConstraintKey::instruction(787);

pub const X86_64_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey =
    RegisterConstraintKey::instruction(715);

pub const X86_64_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey::instruction(717);

pub const X86_64_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey =
    RegisterConstraintKey::instruction(704);

pub const X86_64_LOAD64: RegisterConstraintKey = RegisterConstraintKey::instruction(700);

pub const X86_64_LOAD8_INDEXED: RegisterConstraintKey = RegisterConstraintKey::instruction(703);

pub const X86_64_SAVE_FLOATING_CONTROL: RegisterConstraintKey =
    RegisterConstraintKey::instruction(753);
pub const X86_64_RESTORE_FLOATING_CONTROL: RegisterConstraintKey =
    RegisterConstraintKey::instruction(754);

pub const X86_64_COPY_BYTES: RegisterConstraintKey = RegisterConstraintKey::instruction(736);

pub const X86_64_LOAD_ENCODED_SCALAR: RegisterConstraintKey =
    RegisterConstraintKey::instruction(758);
pub const X86_64_STORE_ENCODED_SCALAR: RegisterConstraintKey =
    RegisterConstraintKey::instruction(759);

pub const X86_64_SCAN_BYTE_SEQUENCE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(755);

pub const X86_64_COMPARE_BYTE_SEQUENCES: RegisterConstraintKey =
    RegisterConstraintKey::instruction(756);

/// Bulk zero fill: preserved destination/count inputs and one early-clobber
/// cursor. Variant 757 is distinct from every AArch64 instruction key too.
pub const X86_64_ZERO_BYTES: RegisterConstraintKey = RegisterConstraintKey::instruction(757);

pub const X86_64_STORE: RegisterConstraintKey = RegisterConstraintKey::instruction(705);

pub const X86_64_ADDRESS_OFFSET: RegisterConstraintKey = RegisterConstraintKey::instruction(706);

pub const X86_64_STORE64: RegisterConstraintKey = RegisterConstraintKey::instruction(701);

pub const X86_64_FRAME_ADDRESS: RegisterConstraintKey = RegisterConstraintKey::instruction(702);

/// `lea r64, [rip+disp32]` naming one byte literal's read-only image storage.
pub const X86_64_BYTE_LITERAL_IMAGE_ADDRESS: RegisterConstraintKey =
    RegisterConstraintKey::instruction(791);

pub(crate) const X86_64_LINUX_SYSTEM_CALL: RegisterConstraintKey =
    RegisterConstraintKey::system_call(0);

pub(crate) const X86_64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey =
    RegisterConstraintKey::inline_assembly(0);

/// Canonical Boolean materialization reads condition state and defines every GPR bit.
pub const X86_64_MATERIALIZE_BOOLEAN: RegisterConstraintKey =
    RegisterConstraintKey::instruction(732);

pub const X86_64_MATERIALIZE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(0);

pub const X86_64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(1);

pub const X86_64_COMPARE_I64_ZERO: RegisterConstraintKey = RegisterConstraintKey::instruction(2);

pub const X86_64_CONDITIONAL_BRANCH: RegisterConstraintKey = RegisterConstraintKey::instruction(3);

/// Flag-transparent three-address i64 addition, realizable with an x86-64 LEA
/// form without introducing a false two-address tie.
pub const X86_64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(4);

/// Flag-transparent `result = left + immediate`, realizable as LEA for the
/// named admitted immediate domain without a destructive two-address tie.
pub const X86_64_ADD_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey::instruction(5);

/// Total unsigned subtraction of every unsigned carrier (a zero-normalized
/// narrow difference borrows exactly when the u64 one does) with an
/// early-clobber result and RFLAGS clobber.
pub const X86_64_SATURATING_SUBTRACT_UNSIGNED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(54);

/// Total unsigned addition clamps to the maximum u64 value.
pub const X86_64_SATURATING_ADD_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(55);

/// Unsigned division consumes an explicit zero RDX input and clobbers the remainder.
pub const X86_64_DIVIDE_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(56);

/// Signed wrapping remainder reads RAX, reads its divisor pinned to RCX so the
/// realized form may zero RDX for the MIN / -1 guard, and defines RAX plus an
/// ordinary late RDX scratch output.
pub const X86_64_REMAINDER_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(57);

/// Saturating addition of every carrier but u64: MOV/ADD into an
/// early-clobber result, clamped through an early-clobber bound scratch
/// (MOVABS/CMP/CMOV against the carrier bounds for narrow carriers, a CMOVO
/// overflow select for i64); clobbers RFLAGS.
pub const X86_64_SATURATING_ADD_CLAMPED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(58);

/// Signed saturating subtraction with the same early-clobber result and
/// bound scratch as the clamped addition.
pub const X86_64_SATURATING_SUBTRACT_CLAMPED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(59);

/// Signed saturating division: CQO/IDIV on RAX with the explicit RDX input
/// of unsigned division. Narrow carriers clamp the only out-of-range quotient
/// MIN / -1 through the carrier maximum held in RDX; the i64 carrier guards
/// that faulting dividend through RDX before dividing. RDX is clobbered.
pub const X86_64_SATURATING_DIVIDE_SIGNED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(60);

/// Saturating multiplication of every carrier but u64: MOV/IMUL into an
/// early-clobber result, then a MOVABS/CMP/CMOV clamp through the
/// early-clobber scratch (narrow carriers), or a CMOVO select of the
/// sign-derived saturated value held in the scratch (i64); clobbers RFLAGS.
pub const X86_64_SATURATING_MULTIPLY_CLAMPED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(65);

/// Saturating u64 multiplication: `MUL` of RAX by the right operand pinned to
/// RCX leaves the high half in RDX and sets CF exactly on overflow; `SBB` and
/// `OR` then saturate RAX. RDX is an ordinary late scratch definition and
/// RFLAGS is clobbered.
pub const X86_64_SATURATING_MULTIPLY_U64: RegisterConstraintKey =
    RegisterConstraintKey::instruction(66);

/// Trapping add, subtract, and multiply of every carrier but the u64
/// multiply: MOV/ADD, SUB, or IMUL into an early-clobber result, then a
/// short branch over UD2 on the overflow or carry flag (64-bit carriers) or
/// on the carrier range check of the result through the early-clobber
/// scratch (narrow carriers); clobbers RFLAGS.
pub const X86_64_TRAPPING_BINARY: RegisterConstraintKey = RegisterConstraintKey::instruction(67);

/// Trapping divide and remainder at every carrier and the trapping u64
/// multiply: the fixed row of the remainder forms. The dividend and result
/// live in RAX, the divisor or right operand is pinned to RCX, and RDX is the
/// high-half definition of CQO, the zeroed DIV input, or MUL's high half.
/// The zero-divisor and MIN / -1 guards branch over UD2 before the divide,
/// so the divide itself never faults; RFLAGS is clobbered.
pub const X86_64_TRAPPING_FIXED_PAIR: RegisterConstraintKey =
    RegisterConstraintKey::instruction(68);

/// Trapping shifts: the value is allocatable, the count is pinned to RCX for
/// CL, and both the result and the round-trip or range-check scratch are
/// early-clobber definitions written while the count and value are live. The
/// count is compared unsigned against the width before any shift; RFLAGS is
/// clobbered.
pub const X86_64_TRAPPING_SHIFT: RegisterConstraintKey = RegisterConstraintKey::instruction(69);

/// Trapping conversion: one allocatable input, then an early-clobber result
/// and an early-clobber scratch for the representability check, which runs
/// before the result is written; RFLAGS is clobbered.
pub const X86_64_TRAPPING_CONVERT: RegisterConstraintKey = RegisterConstraintKey::instruction(70);

/// Runtime port operands and read results use ordinary allocatable GPRs.
/// Architectural inputs are snapshotted before scratch registers change.
pub const X86_64_PORT_REGISTER_READ: RegisterConstraintKey =
    RegisterConstraintKey::instruction(803);

pub const X86_64_PORT_REGISTER_WRITE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(804);

pub const X86_64_MODEL_SPECIFIC_READ: RegisterConstraintKey =
    RegisterConstraintKey::instruction(805);

pub const X86_64_MODEL_SPECIFIC_WRITE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(806);

pub const X86_64_PROCESSOR_HALT: RegisterConstraintKey = RegisterConstraintKey::instruction(807);

pub const X86_64_INTERRUPT_CONTROL: RegisterConstraintKey = RegisterConstraintKey::instruction(808);

pub const X86_64_PORT_WRITE: RegisterConstraintKey = RegisterConstraintKey::instruction(802);

pub const X86_64_ATOMIC_FENCE: RegisterConstraintKey = RegisterConstraintKey::instruction(801);
pub const X86_64_ATOMIC_LOAD: RegisterConstraintKey = RegisterConstraintKey::instruction(71);

/// Atomic store of every width: `MOV` into the cell then `MFENCE`, so the
/// value input is never destroyed and store-load ordering holds. Never
/// writes RFLAGS.
pub const X86_64_ATOMIC_STORE: RegisterConstraintKey = RegisterConstraintKey::instruction(72);

/// Atomic update returning the observed prior value: `XCHG` for swap,
/// `LOCK XADD` for fetch-add, `NEG` plus `LOCK XADD` for fetch-sub — the
/// operand register and the observed result are one tied in/out register.
/// `LOCK XADD` writes RFLAGS; `XCHG` and `NEG` are listed clobbers so the
/// row is honest about the flags the sequence may define.
pub const X86_64_ATOMIC_UPDATE: RegisterConstraintKey = RegisterConstraintKey::instruction(73);

/// Bitwise fetch uses a compare-exchange retry sequence. RAX holds the
/// current comparison, while fixed caller-volatile homes preserve the address
/// and authored mask and separate the returned prior from the replacement.
pub const X86_64_ATOMIC_BITWISE_UPDATE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(747);

/// Atomic compare-exchange: `LOCK CMPXCHG` reads the expected value from RAX
/// and leaves the observed prior cell value in RAX; the replacement is an
/// ordinary allocatable input. RFLAGS is clobbered.
pub const X86_64_ATOMIC_COMPARE_EXCHANGE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(74);

/// Exact `result = left * right` three-address pseudo. Its realization must be
/// alias-safe for every allocator result: `IMUL result, right` when the result
/// aliases the left input, `IMUL result, left` when it aliases only the right
/// input, `IMUL result, result` reads the aliased input when all three share a
/// view, and `MOV; IMUL` otherwise. `IMUL` defines RFLAGS.
pub const X86_64_MULTIPLY_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(61);

/// Signed i64 wrapping division. `IDIV` faults on MIN / -1 while the wrapping
/// semantics require i64::MIN, so the realization guards that divisor: it
/// initializes the result to the wrapping quotient, compares the divisor
/// against -1, branches over the divide on equality, otherwise sign-extends
/// the dividend with CQO and executes IDIV. The dividend and quotient live in
/// RAX, RDX is the sign-extension clobber, and the compare defines RFLAGS.
pub const X86_64_DIVIDE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(62);

/// Unsigned u64 remainder on the fixed RAX:RDX pair: the dividend occupies
/// RAX, RDX is zeroed as the high input, DIV leaves the remainder in RDX, and
/// a move returns it to the result view. Both fixed registers are clobbered
/// and RFLAGS is undefined across the divide.
pub const X86_64_REMAINDER_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(63);

/// Two-source variable-count shift (SHL/SHR/SAR): the value is allocatable,
/// the count is pinned to RCX because the hardware reads CL, and the result
/// is an early-clobber definition so it can never share the live count home.
/// The realized form copies the value into the result before shifting it in
/// place; RFLAGS is clobbered.
pub const X86_64_SHIFT_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(64);

/// Exact `result = left - right` three-address pseudo. Its realization must be
/// alias-safe for every allocator result: `XOR result, result` when both inputs
/// share a view, `SUB` when the result is only the left input, `NEG; ADD` when
/// it is only the right input, and `MOV; SUB` otherwise. Those alternatives do
/// not preserve one common flags value, so the row explicitly clobbers RFLAGS.
pub const X86_64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(6);

/// Flag-transparent `result = left - immediate`, realized as an x86-64 LEA
/// with the negated admitted U12 displacement and no destructive tie.
pub const X86_64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(7);

/// Two-input i64 comparison. Both operands are read and RFLAGS is defined.
pub const X86_64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(8);

/// One-input i64 comparison against the encoded unsigned immediate. The
/// operand is read and RFLAGS is defined, matching `cmp r64, imm32`.
pub const X86_64_COMPARE_I64_IMMEDIATE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(10);

/// Unconditional relative control without a condition-register dependency.
pub const X86_64_JUMP: RegisterConstraintKey = RegisterConstraintKey::instruction(9);

/// The named ordinary x86-64 rows: every constraint key the target mints as
/// its own constant instead of as a member of a call or return family.
///
/// The ordinary rows are deliberately limited to the baseline operations
/// required by a register-passed scalar conditional-return CFG plus the first
/// arithmetic row needed by the pressure vertical. This is not a claim that
/// the target's ordinary instruction inventory is complete.
pub(crate) const X86_64_ORDINARY_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 90] = [
    X86_64_SYSTEM_V_CALL,
    X86_64_MICROSOFT_CALL,
    X86_64_SYSTEM_V_RETURN,
    X86_64_MICROSOFT_RETURN,
    X86_64_SYSTEM_V_RETURN_UNIT,
    X86_64_MICROSOFT_RETURN_UNIT,
    X86_64_LINUX_SYSTEM_CALL,
    X86_64_INLINE_ASSEMBLY_DEFAULT,
    X86_64_MATERIALIZE_I64,
    X86_64_COPY_I64,
    X86_64_COMPARE_I64_ZERO,
    X86_64_CONDITIONAL_BRANCH,
    X86_64_ADD_I64,
    X86_64_ADD_I64_IMMEDIATE,
    X86_64_SUBTRACT_I64,
    X86_64_SUBTRACT_I64_IMMEDIATE,
    X86_64_COMPARE_I64,
    X86_64_JUMP,
    X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_SATURATING_SUBTRACT_UNSIGNED,
    X86_64_SATURATING_ADD_U64,
    X86_64_DIVIDE_U64,
    X86_64_REMAINDER_I64,
    X86_64_SATURATING_ADD_CLAMPED,
    X86_64_SATURATING_SUBTRACT_CLAMPED,
    X86_64_SATURATING_DIVIDE_SIGNED,
    X86_64_MULTIPLY_I64,
    X86_64_DIVIDE_I64,
    X86_64_REMAINDER_U64,
    X86_64_SHIFT_I64,
    X86_64_SATURATING_MULTIPLY_CLAMPED,
    X86_64_SATURATING_MULTIPLY_U64,
    X86_64_TRAPPING_BINARY,
    X86_64_TRAPPING_FIXED_PAIR,
    X86_64_TRAPPING_SHIFT,
    X86_64_TRAPPING_CONVERT,
    X86_64_ATOMIC_LOAD,
    X86_64_ATOMIC_STORE,
    X86_64_ATOMIC_UPDATE,
    X86_64_ATOMIC_COMPARE_EXCHANGE,
    X86_64_LOAD64,
    X86_64_STORE64,
    X86_64_FRAME_ADDRESS,
    X86_64_LOAD8_INDEXED,
    X86_64_HOSTED_WRITE_BYTE_I32,
    X86_64_STORE,
    X86_64_ADDRESS_OFFSET,
    X86_64_FLOAT32_TO_BITS,
    X86_64_FLOAT64_TO_BITS,
    X86_64_BITS_TO_FLOAT32,
    X86_64_BITS_TO_FLOAT64,
    X86_64_LOAD32,
    X86_64_HOSTED_EXIT_PROCESS_I32,
    X86_64_HOSTED_READ_BYTE,
    X86_64_LOAD8,
    X86_64_LOAD16,
    X86_64_MATERIALIZE_BOOLEAN,
    X86_64_COPY_BYTES,
    X86_64_ATOMIC_BITWISE_UPDATE,
    X86_64_SAVE_FLOATING_CONTROL,
    X86_64_RESTORE_FLOATING_CONTROL,
    X86_64_SCAN_BYTE_SEQUENCE,
    X86_64_COMPARE_BYTE_SEQUENCES,
    X86_64_ZERO_BYTES,
    X86_64_LOAD_ENCODED_SCALAR,
    X86_64_STORE_ENCODED_SCALAR,
    X86_64_CRASH,
    X86_64_IEEE_FLOAT_ARITHMETIC,
    X86_64_IEEE_FLOAT_UNARY,
    X86_64_IEEE_FLOAT_TERNARY,
    X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
    X86_64_FLOAT_RESULT_TRAPPING,
    X86_64_FLOAT_RESULT_SATURATING,
    X86_64_BYTE_LITERAL_IMAGE_ADDRESS,
    X86_64_IEEE_FLOAT_DIRECTED_BINARY,
    X86_64_IEEE_FLOAT_DIRECTED_UNARY,
    X86_64_ATOMIC_FENCE,
    X86_64_PORT_WRITE,
    X86_64_PORT_REGISTER_READ,
    X86_64_PORT_REGISTER_WRITE,
    X86_64_MODEL_SPECIFIC_READ,
    X86_64_MODEL_SPECIFIC_WRITE,
    X86_64_PROCESSOR_HALT,
    X86_64_INTERRUPT_CONTROL,
    X86_64_PLACED_READ,
    X86_64_PLACED_WRITE,
    X86_64_FLAGS_SNAPSHOT,
    X86_64_CONTROL_REGISTER_READ,
    X86_64_CONTROL_REGISTER_WRITE,
    X86_64_FLAGS_RESTORE,
];

/// Closed v1 inventory owned by the x86-64 target: the named ordinary rows
/// plus every call and return family of the target.
///
/// Each family is extended from the key owner that mints it, so the inventory
/// cannot drift from the rows the catalog builds out of the same keys.
pub(crate) fn x86_64_required_register_constraints() -> Vec<RegisterConstraintKey> {
    let mut required = X86_64_ORDINARY_REGISTER_CONSTRAINTS.to_vec();
    required.extend(super::call_keys::x86_64_system_v_register_call_keys());
    required.extend(super::call_keys::x86_64_microsoft_register_call_keys());
    required.extend(super::call_keys::x86_64_system_v_register_unit_call_keys());
    required.extend(super::call_keys::x86_64_microsoft_register_unit_call_keys());
    required.extend(super::call_keys::x86_64_system_v_aggregate_call_keys());
    required.extend(super::call_keys::x86_64_system_v_aggregate_return_keys());
    required.extend(super::call_keys::x86_64_microsoft_aggregate_call_keys());
    required.extend(super::call_keys::x86_64_microsoft_aggregate_return_keys());
    required.extend(super::call_keys::x86_64_system_v_normalized_foreign_call_keys());
    required.extend(super::call_keys::x86_64_microsoft_normalized_foreign_call_keys());
    required.extend(super::mixed_calls::x86_64_system_v_mixed_unit_call_keys());
    required.extend(super::mixed_calls::x86_64_microsoft_mixed_unit_call_keys());
    required.extend(super::mixed_aggregate_calls::x86_64_system_v_mixed_aggregate_call_keys());
    required.extend(super::mixed_aggregate_calls::x86_64_microsoft_mixed_aggregate_call_keys());
    for microsoft in [false, true] {
        required.extend(x86_64_float_scalar_call_keys(microsoft));
        required.extend(x86_64_float_scalar_return_keys(microsoft));
        required.extend(x86_64_indirect_aggregate_call_keys(microsoft));
    }
    required.extend(super::system_calls::x86_64_linux_system_call_keys());
    required.extend([X86_64_LOAD_PACKED, X86_64_STORE_PACKED]);
    required.sort_unstable();
    required
}

pub const X86_64_PLACED_READ: RegisterConstraintKey = RegisterConstraintKey::instruction(900);

pub const X86_64_PLACED_WRITE: RegisterConstraintKey = RegisterConstraintKey::instruction(901);

pub const X86_64_FLAGS_SNAPSHOT: RegisterConstraintKey = RegisterConstraintKey::instruction(902);

pub const X86_64_FLAGS_RESTORE: RegisterConstraintKey = RegisterConstraintKey::instruction(905);

pub const X86_64_CONTROL_REGISTER_READ: RegisterConstraintKey =
    RegisterConstraintKey::instruction(903);

pub const X86_64_CONTROL_REGISTER_WRITE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(904);
