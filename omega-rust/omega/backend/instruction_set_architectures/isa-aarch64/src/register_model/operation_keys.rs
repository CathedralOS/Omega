//! The constraint keys of the ordinary AArch64 operations: loads, stores,
//! packed memory, float bit casts, hosted calls, arithmetic, comparisons and
//! branches, and the closed inventory every catalog must carry.

use crate::register_model::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, AARCH64_AAPCS64_RETURN,
    AARCH64_AAPCS64_RETURN_UNIT, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN,
    AARCH64_DARWIN_RETURN_UNIT,
};
use register_model::{RegisterConstraintFamily, RegisterConstraintKey};

pub const AARCH64_COPY_BYTES: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 752,
};

pub const AARCH64_LOAD8: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 730,
};

/// Exact odd-width byte loads with an explicit early-clobber scratch operand.
pub const AARCH64_LOAD_PACKED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 750,
};

/// Exact odd-width byte stores preserving both inputs with explicit scratch.
pub const AARCH64_STORE_PACKED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 751,
};

pub const AARCH64_LOAD16: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 731,
};

pub const AARCH64_LOAD32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 714,
};

pub const AARCH64_FLOAT32_TO_BITS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 710,
};

pub const AARCH64_FLOAT64_TO_BITS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 711,
};

pub const AARCH64_BITS_TO_FLOAT32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 712,
};

pub const AARCH64_BITS_TO_FLOAT64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 713,
};

pub const AARCH64_LINUX_SYSTEM_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::SystemCall,
    variant: 0,
};

pub const AARCH64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::InlineAssembly,
    variant: 0,
};

/// Canonical Boolean materialization reads condition state and defines every GPR bit.
pub const AARCH64_MATERIALIZE_BOOLEAN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 732,
};

pub const AARCH64_MATERIALIZE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 0,
};

pub const AARCH64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 1,
};

pub const AARCH64_COMPARE_I64_ZERO: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 2,
};

pub const AARCH64_CONDITIONAL_BRANCH: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 3,
};

/// Flag-transparent three-address i64 addition, matching the ordinary AArch64
/// register ADD form.
pub const AARCH64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 4,
};

/// Flag-transparent `result = left + immediate`, matching the AArch64 ADD
/// immediate form for the named admitted immediate domain.
pub const AARCH64_ADD_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 5,
};

/// Unsigned saturating subtraction for every unsigned carrier: SUBS followed
/// by CSEL clamps at zero on borrow; defines NZCV.
pub const AARCH64_SATURATING_SUBTRACT_UNSIGNED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 54,
};

/// Total unsigned addition clamps to the maximum u64 value.
pub const AARCH64_SATURATING_ADD_U64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 55,
};

/// Unsigned division uses the ordinary three-address UDIV register form.
pub const AARCH64_DIVIDE_U64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 56,
};

/// Signed remainder preserves both inputs until MSUB consumes the SDIV quotient.
pub const AARCH64_REMAINDER_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 57,
};

/// Clamped saturating addition (every carrier but u64): arithmetic into an
/// early-clobber result, then a bound held in an early-clobber scratch
/// selects the saturated value (CMP/CSEL per carrier bound, or the i64
/// overflow-flag select); defines NZCV.
pub const AARCH64_SATURATING_ADD_CLAMPED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 58,
};

/// Clamped saturating subtraction for the signed carriers, with the same
/// operand shape as clamped addition.
pub const AARCH64_SATURATING_SUBTRACT_CLAMPED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 59,
};

/// Signed saturating division: SDIV whose only out-of-range quotient,
/// MIN / -1, is clamped (narrow carriers) or recognized and replaced (i64)
/// through the scratch-held bound; defines NZCV.
pub const AARCH64_SATURATING_DIVIDE_SIGNED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 60,
};

/// Flag-transparent three-address exact i64 multiplication, matching the
/// ordinary AArch64 `MUL` register form.
pub const AARCH64_MULTIPLY_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 61,
};

/// Flag-transparent three-address i64 wrapping division, matching the
/// ordinary AArch64 `SDIV` register form whose i64::MIN / -1 quotient
/// already wraps to i64::MIN.
pub const AARCH64_DIVIDE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 62,
};

/// Three-address u64 remainder: `UDIV` into an early-clobber result scratch,
/// then `MSUB` recovers the remainder in place.
pub const AARCH64_REMAINDER_U64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 63,
};

/// Flag-transparent three-address exact i64 subtraction, matching the
/// ordinary AArch64 `SUB` register form.
pub const AARCH64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 6,
};

/// Flag-transparent `result = left - immediate`, matching the AArch64 SUB
/// immediate form for the named admitted U12 domain.
pub const AARCH64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 7,
};

/// Two-input i64 comparison. Both operands are read and NZCV is defined.
pub const AARCH64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 8,
};

/// One-input i64 comparison against the encoded U12 immediate. The operand is
/// read and NZCV is defined, matching `subs xzr, xN, #imm12`.
pub const AARCH64_COMPARE_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 18,
};

/// Unconditional relative control without a condition-register dependency.
pub const AARCH64_JUMP: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 9,
};

/// Process exit using the Linux syscall register convention.
pub const AARCH64_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 715,
};

pub const AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 716,
};

pub const AARCH64_DARWIN_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 718,
};

pub const AARCH64_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 717,
};

pub const AARCH64_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 14,
};

/// The same hosted byte operation using Darwin's syscall register convention.
pub const AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 17,
};

pub const AARCH64_LOAD64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 10,
};

/// One byte read through a base and runtime index, zero extended to 64 bits.
pub const AARCH64_LOAD8_INDEXED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 11,
};

pub const AARCH64_STORE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 15,
};

pub const AARCH64_ADDRESS_OFFSET: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 16,
};

pub const AARCH64_STORE64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 12,
};

pub const AARCH64_FRAME_ADDRESS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 13,
};

/// Closed baseline constraint inventory owned by the AArch64 target.
/// Includes scalar control, arithmetic, calls, and pointer loads; other
/// ordinary and feature-specific instruction rows remain absent.
pub const AARCH64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 88] = [
    AARCH64_AAPCS64_CALL,
    AARCH64_DARWIN_CALL,
    AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64,
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 3,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 4,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 5,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 6,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 7,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 8,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 9,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 10,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 11,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 12,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 13,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 14,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 15,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 16,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 17,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 18,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 19,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 700,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 701,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 702,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 703,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 704,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 705,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 706,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 707,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 708,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 720,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 721,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 722,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 723,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 724,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 725,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 726,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 727,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 728,
    },
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
];
