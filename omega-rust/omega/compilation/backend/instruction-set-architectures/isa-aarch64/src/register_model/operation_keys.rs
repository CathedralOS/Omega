//! The constraint keys of the ordinary AArch64 operations: loads, stores,
//! packed memory, float bit casts, hosted calls, arithmetic, comparisons and
//! branches, and the closed inventory every catalog must carry.

use crate::register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT,
};
use register_model::RegisterConstraintKey;

pub const AARCH64_ATOMIC_FENCE: RegisterConstraintKey = RegisterConstraintKey::instruction(800);
pub const AARCH64_ATOMIC_LOAD: RegisterConstraintKey = RegisterConstraintKey::instruction(781);
pub const AARCH64_ATOMIC_STORE: RegisterConstraintKey = RegisterConstraintKey::instruction(782);
pub const AARCH64_ATOMIC_UPDATE: RegisterConstraintKey = RegisterConstraintKey::instruction(783);
pub const AARCH64_ATOMIC_COMPARE_EXCHANGE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(784);

pub const AARCH64_CRASH: RegisterConstraintKey = RegisterConstraintKey::instruction(780);

pub const AARCH64_SAVE_FLOATING_CONTROL: RegisterConstraintKey =
    RegisterConstraintKey::instruction(753);
pub const AARCH64_RESTORE_FLOATING_CONTROL: RegisterConstraintKey =
    RegisterConstraintKey::instruction(754);

/// Nearest-even IEEE operations on general-register payloads: two inputs and
/// one result, with FPCR read and scratch `v16`..`v18` clobbered.
pub const AARCH64_IEEE_FLOAT_ARITHMETIC: RegisterConstraintKey =
    RegisterConstraintKey::instruction(785);

/// The same IEEE forms with one general-register input (negation, square
/// root and conversions) and one result.
pub const AARCH64_IEEE_FLOAT_UNARY: RegisterConstraintKey = RegisterConstraintKey::instruction(786);

/// Three general-register inputs and one result, with the same floating
/// controls and scratch-register effects as the unary and binary forms.
pub const AARCH64_IEEE_FLOAT_TERNARY: RegisterConstraintKey =
    RegisterConstraintKey::instruction(787);

/// Raw IEEE predicates use no floating controls, and clobber NZCV and v16..v18.
pub const AARCH64_FLOAT_RESULT_TRAPPING: RegisterConstraintKey =
    RegisterConstraintKey::instruction(788);
pub const AARCH64_FLOAT_RESULT_SATURATING: RegisterConstraintKey =
    RegisterConstraintKey::instruction(789);

pub const AARCH64_COPY_BYTES: RegisterConstraintKey = RegisterConstraintKey::instruction(752);

pub const AARCH64_LOAD_ENCODED_SCALAR: RegisterConstraintKey =
    RegisterConstraintKey::instruction(797);
pub const AARCH64_STORE_ENCODED_SCALAR: RegisterConstraintKey =
    RegisterConstraintKey::instruction(798);

pub const AARCH64_SCAN_BYTE_SEQUENCE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(794);

pub const AARCH64_COMPARE_BYTE_SEQUENCES: RegisterConstraintKey =
    RegisterConstraintKey::instruction(795);

/// Bulk zero fill: preserved destination/count inputs and one early-clobber
/// cursor. Variant 796 is distinct from every x86-64 instruction key too.
pub const AARCH64_ZERO_BYTES: RegisterConstraintKey = RegisterConstraintKey::instruction(796);

pub const AARCH64_LOAD8: RegisterConstraintKey = RegisterConstraintKey::instruction(730);

/// Exact odd-width byte loads with an explicit early-clobber scratch operand.
pub const AARCH64_LOAD_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(750);

/// Exact odd-width byte stores preserving both inputs with explicit scratch.
pub const AARCH64_STORE_PACKED: RegisterConstraintKey = RegisterConstraintKey::instruction(751);

pub const AARCH64_LOAD16: RegisterConstraintKey = RegisterConstraintKey::instruction(731);

pub const AARCH64_LOAD32: RegisterConstraintKey = RegisterConstraintKey::instruction(714);

pub const AARCH64_FLOAT32_TO_BITS: RegisterConstraintKey = RegisterConstraintKey::instruction(710);

pub const AARCH64_FLOAT64_TO_BITS: RegisterConstraintKey = RegisterConstraintKey::instruction(711);

pub const AARCH64_BITS_TO_FLOAT32: RegisterConstraintKey = RegisterConstraintKey::instruction(712);

pub const AARCH64_BITS_TO_FLOAT64: RegisterConstraintKey = RegisterConstraintKey::instruction(713);

pub(crate) const AARCH64_LINUX_SYSTEM_CALL: RegisterConstraintKey =
    RegisterConstraintKey::system_call(0);

pub(crate) const AARCH64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey =
    RegisterConstraintKey::inline_assembly(0);

/// Canonical Boolean materialization reads condition state and defines every GPR bit.
pub const AARCH64_MATERIALIZE_BOOLEAN: RegisterConstraintKey =
    RegisterConstraintKey::instruction(732);

pub const AARCH64_MATERIALIZE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(0);

pub const AARCH64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(1);

pub const AARCH64_COMPARE_I64_ZERO: RegisterConstraintKey = RegisterConstraintKey::instruction(2);

pub const AARCH64_CONDITIONAL_BRANCH: RegisterConstraintKey = RegisterConstraintKey::instruction(3);

/// Flag-transparent three-address i64 addition, matching the ordinary AArch64
/// register ADD form.
pub const AARCH64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(4);

/// Flag-transparent `result = left + immediate`, matching the AArch64 ADD
/// immediate form for the named admitted immediate domain.
pub const AARCH64_ADD_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey::instruction(5);

/// Unsigned saturating subtraction for every unsigned carrier: SUBS followed
/// by CSEL clamps at zero on borrow; defines NZCV.
pub const AARCH64_SATURATING_SUBTRACT_UNSIGNED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(54);

/// Total unsigned addition clamps to the maximum u64 value.
pub const AARCH64_SATURATING_ADD_U64: RegisterConstraintKey =
    RegisterConstraintKey::instruction(55);

/// Unsigned division uses the ordinary three-address UDIV register form.
pub const AARCH64_DIVIDE_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(56);

/// Signed remainder preserves both inputs until MSUB consumes the SDIV quotient.
pub const AARCH64_REMAINDER_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(57);

/// Clamped saturating addition (every carrier but u64): arithmetic into an
/// early-clobber result, then a bound held in an early-clobber scratch
/// selects the saturated value (CMP/CSEL per carrier bound, or the i64
/// overflow-flag select); defines NZCV.
pub const AARCH64_SATURATING_ADD_CLAMPED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(58);

/// Clamped saturating subtraction for the signed carriers, with the same
/// operand shape as clamped addition.
pub const AARCH64_SATURATING_SUBTRACT_CLAMPED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(59);

/// Signed saturating division: SDIV whose only out-of-range quotient,
/// MIN / -1, is clamped (narrow carriers) or recognized and replaced (i64)
/// through the scratch-held bound; defines NZCV.
pub const AARCH64_SATURATING_DIVIDE_SIGNED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(60);

/// Saturating multiplication of every carrier but u64, with the operand
/// shape of clamped addition: `MUL` into the early-clobber result, then a
/// clamp through the early-clobber scratch (narrow carriers), or the i64
/// `SMULH` high half in the scratch deciding overflow and the saturated
/// value; defines NZCV.
pub const AARCH64_SATURATING_MULTIPLY_CLAMPED: RegisterConstraintKey =
    RegisterConstraintKey::instruction(65);

/// Saturating u64 multiplication: `UMULH` into the early-clobber scratch and
/// `MUL` into the early-clobber result, then `CSINV` selects u64::MAX when
/// the high half is nonzero; defines NZCV.
pub const AARCH64_SATURATING_MULTIPLY_U64: RegisterConstraintKey =
    RegisterConstraintKey::instruction(66);

/// Trapping add, subtract and multiply: two inputs, an early-clobber result,
/// and an early-clobber scratch (the i64 product's high half); every
/// Trapping row clobbers NZCV and ends in the inline `brk #0` skip.
pub const AARCH64_TRAPPING_BINARY: RegisterConstraintKey = RegisterConstraintKey::instruction(67);

/// Trapping divide and remainder at every carrier and the u64 multiply. The
/// operand shape matches `AARCH64_TRAPPING_BINARY`: AArch64 needs no fixed
/// pair, the role exists because x86-64 pins RAX/RCX/RDX for the same forms.
pub const AARCH64_TRAPPING_FIXED_PAIR: RegisterConstraintKey =
    RegisterConstraintKey::instruction(68);

/// Trapping shifts: value and count read, early-clobber result and scratch.
pub const AARCH64_TRAPPING_SHIFT: RegisterConstraintKey = RegisterConstraintKey::instruction(69);

/// Trapping conversion: one input, early-clobber result and scratch.
pub const AARCH64_TRAPPING_CONVERT: RegisterConstraintKey = RegisterConstraintKey::instruction(70);

/// Flag-transparent three-address exact i64 multiplication, matching the
/// ordinary AArch64 `MUL` register form.
pub const AARCH64_MULTIPLY_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(61);

/// Flag-transparent three-address i64 wrapping division, matching the
/// ordinary AArch64 `SDIV` register form whose i64::MIN / -1 quotient
/// already wraps to i64::MIN.
pub const AARCH64_DIVIDE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(62);

/// Three-address u64 remainder: `UDIV` into an early-clobber result scratch,
/// then `MSUB` recovers the remainder in place.
pub const AARCH64_REMAINDER_U64: RegisterConstraintKey = RegisterConstraintKey::instruction(63);

/// Flag-transparent three-address variable-count shift. The register forms
/// `LSLV`, `LSRV`, and `ASRV` all share the shape `Xd = Xn shifted by the
/// low six bits of Xm`, so the count needs no pinning or masking and every
/// operand stays allocatable.
pub const AARCH64_SHIFT_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(64);

/// Flag-transparent three-address exact i64 subtraction, matching the
/// ordinary AArch64 `SUB` register form.
pub const AARCH64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(6);

/// Flag-transparent `result = left - immediate`, matching the AArch64 SUB
/// immediate form for the named admitted U12 domain.
pub const AARCH64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(7);

/// Two-input i64 comparison. Both operands are read and NZCV is defined.
pub const AARCH64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey::instruction(8);

/// One-input i64 comparison against the encoded U12 immediate. The operand is
/// read and NZCV is defined, matching `subs xzr, xN, #imm12`.
pub const AARCH64_COMPARE_I64_IMMEDIATE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(18);

/// Unconditional relative control without a condition-register dependency.
pub const AARCH64_JUMP: RegisterConstraintKey = RegisterConstraintKey::instruction(9);

/// Process exit using the Linux syscall register convention.
pub const AARCH64_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey =
    RegisterConstraintKey::instruction(715);

pub const AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey =
    RegisterConstraintKey::instruction(716);

pub const AARCH64_DARWIN_HOSTED_READ_BYTE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(718);

pub const AARCH64_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey::instruction(717);

pub const AARCH64_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey =
    RegisterConstraintKey::instruction(14);

/// The same hosted byte operation using Darwin's syscall register convention.
pub const AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey =
    RegisterConstraintKey::instruction(17);

pub const AARCH64_LOAD64: RegisterConstraintKey = RegisterConstraintKey::instruction(10);

/// One byte read through a base and runtime index, zero extended to 64 bits.
pub const AARCH64_LOAD8_INDEXED: RegisterConstraintKey = RegisterConstraintKey::instruction(11);

pub const AARCH64_STORE: RegisterConstraintKey = RegisterConstraintKey::instruction(15);

pub const AARCH64_ADDRESS_OFFSET: RegisterConstraintKey = RegisterConstraintKey::instruction(16);

pub const AARCH64_STORE64: RegisterConstraintKey = RegisterConstraintKey::instruction(12);

pub const AARCH64_FRAME_ADDRESS: RegisterConstraintKey = RegisterConstraintKey::instruction(13);

/// `adrp`/`add` pair naming one byte literal's read-only image storage.
pub const AARCH64_BYTE_LITERAL_IMAGE_ADDRESS: RegisterConstraintKey =
    RegisterConstraintKey::instruction(791);

/// Closed baseline constraint inventory owned by the AArch64 target.
/// Includes scalar control, arithmetic, calls, and pointer loads; other
/// ordinary and feature-specific instruction rows remain absent.
pub(crate) const AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 118] = [
    AARCH64_AAPCS64_CALL,
    AARCH64_DARWIN_CALL,
    AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64,
    RegisterConstraintKey::call(3),
    RegisterConstraintKey::call(4),
    RegisterConstraintKey::call(5),
    RegisterConstraintKey::call(6),
    RegisterConstraintKey::call(7),
    RegisterConstraintKey::call(8),
    RegisterConstraintKey::call(9),
    RegisterConstraintKey::call(10),
    RegisterConstraintKey::call(11),
    RegisterConstraintKey::call(12),
    RegisterConstraintKey::call(13),
    RegisterConstraintKey::call(14),
    RegisterConstraintKey::call(15),
    RegisterConstraintKey::call(16),
    RegisterConstraintKey::call(17),
    RegisterConstraintKey::call(18),
    RegisterConstraintKey::call(19),
    RegisterConstraintKey::call(700),
    RegisterConstraintKey::call(701),
    RegisterConstraintKey::call(702),
    RegisterConstraintKey::call(703),
    RegisterConstraintKey::call(704),
    RegisterConstraintKey::call(705),
    RegisterConstraintKey::call(706),
    RegisterConstraintKey::call(707),
    RegisterConstraintKey::call(708),
    RegisterConstraintKey::call(720),
    RegisterConstraintKey::call(721),
    RegisterConstraintKey::call(722),
    RegisterConstraintKey::call(723),
    RegisterConstraintKey::call(724),
    RegisterConstraintKey::call(725),
    RegisterConstraintKey::call(726),
    RegisterConstraintKey::call(727),
    RegisterConstraintKey::call(728),
    AARCH64_AAPCS64_RETURN,
    AARCH64_DARWIN_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT,
    AARCH64_DARWIN_RETURN_UNIT,
    AARCH64_LINUX_SYSTEM_CALL,
    AARCH64_INLINE_ASSEMBLY_DEFAULT,
    AARCH64_MATERIALIZE_I64,
    AARCH64_COPY_I64,
    AARCH64_COMPARE_I64_ZERO,
    AARCH64_CONDITIONAL_BRANCH,
    AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE,
    AARCH64_SUBTRACT_I64,
    AARCH64_SUBTRACT_I64_IMMEDIATE,
    AARCH64_COMPARE_I64,
    AARCH64_JUMP,
    AARCH64_LOAD64,
    AARCH64_LOAD8_INDEXED,
    AARCH64_STORE64,
    AARCH64_FRAME_ADDRESS,
    AARCH64_HOSTED_WRITE_BYTE_I32,
    AARCH64_STORE,
    AARCH64_ADDRESS_OFFSET,
    AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32,
    AARCH64_COMPARE_I64_IMMEDIATE,
    AARCH64_SATURATING_SUBTRACT_UNSIGNED,
    AARCH64_SATURATING_ADD_U64,
    AARCH64_DIVIDE_U64,
    AARCH64_REMAINDER_I64,
    AARCH64_SATURATING_ADD_CLAMPED,
    AARCH64_SATURATING_SUBTRACT_CLAMPED,
    AARCH64_SATURATING_DIVIDE_SIGNED,
    AARCH64_MULTIPLY_I64,
    AARCH64_DIVIDE_I64,
    AARCH64_REMAINDER_U64,
    AARCH64_SHIFT_I64,
    AARCH64_SATURATING_MULTIPLY_CLAMPED,
    AARCH64_SATURATING_MULTIPLY_U64,
    AARCH64_TRAPPING_BINARY,
    AARCH64_TRAPPING_FIXED_PAIR,
    AARCH64_TRAPPING_SHIFT,
    AARCH64_TRAPPING_CONVERT,
    AARCH64_FLOAT32_TO_BITS,
    AARCH64_FLOAT64_TO_BITS,
    AARCH64_BITS_TO_FLOAT32,
    AARCH64_BITS_TO_FLOAT64,
    AARCH64_LOAD32,
    AARCH64_HOSTED_EXIT_PROCESS_I32,
    AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32,
    AARCH64_HOSTED_READ_BYTE,
    AARCH64_DARWIN_HOSTED_READ_BYTE,
    AARCH64_LOAD8,
    AARCH64_LOAD16,
    AARCH64_MATERIALIZE_BOOLEAN,
    AARCH64_LOAD_PACKED,
    AARCH64_STORE_PACKED,
    AARCH64_COPY_BYTES,
    AARCH64_SAVE_FLOATING_CONTROL,
    AARCH64_RESTORE_FLOATING_CONTROL,
    AARCH64_CRASH,
    AARCH64_ATOMIC_LOAD,
    AARCH64_ATOMIC_STORE,
    AARCH64_ATOMIC_UPDATE,
    AARCH64_ATOMIC_COMPARE_EXCHANGE,
    AARCH64_IEEE_FLOAT_ARITHMETIC,
    AARCH64_IEEE_FLOAT_UNARY,
    AARCH64_IEEE_FLOAT_TERNARY,
    AARCH64_FLOAT_RESULT_TRAPPING,
    AARCH64_FLOAT_RESULT_SATURATING,
    AARCH64_BYTE_LITERAL_IMAGE_ADDRESS,
    AARCH64_SCAN_BYTE_SEQUENCE,
    AARCH64_COMPARE_BYTE_SEQUENCES,
    AARCH64_ZERO_BYTES,
    AARCH64_LOAD_ENCODED_SCALAR,
    AARCH64_STORE_ENCODED_SCALAR,
    AARCH64_ATOMIC_FENCE,
    AARCH64_PLACED_READ,
    AARCH64_PLACED_WRITE,
    AARCH64_SYSTEM_REGISTER_READ,
    AARCH64_SYSTEM_REGISTER_WRITE,
];

pub const AARCH64_SYSTEM_REGISTER_READ: RegisterConstraintKey =
    RegisterConstraintKey::instruction(902);

pub const AARCH64_SYSTEM_REGISTER_WRITE: RegisterConstraintKey =
    RegisterConstraintKey::instruction(903);

pub const AARCH64_PLACED_READ: RegisterConstraintKey = RegisterConstraintKey::instruction(900);

pub const AARCH64_PLACED_WRITE: RegisterConstraintKey = RegisterConstraintKey::instruction(901);
