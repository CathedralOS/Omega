//! The constraint keys of the ordinary x86-64 operations: loads, stores,
//! float bit casts, hosted calls, arithmetic, comparisons and branches, and
//! the closed inventory every catalog must carry.

use crate::register_model::{
    X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_CALL_UNIT, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
    X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
};
use register_model::{RegisterConstraintFamily, RegisterConstraintKey};

pub const X86_64_LOAD8: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 730,
};

pub const X86_64_LOAD16: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 731,
};

pub const X86_64_LOAD32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 714,
};

pub const X86_64_FLOAT32_TO_BITS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 710,
};

pub const X86_64_FLOAT64_TO_BITS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 711,
};

pub const X86_64_BITS_TO_FLOAT32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 712,
};

pub const X86_64_BITS_TO_FLOAT64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 713,
};

pub const X86_64_HOSTED_EXIT_PROCESS_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 715,
};

pub const X86_64_HOSTED_READ_BYTE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 717,
};

pub const X86_64_HOSTED_WRITE_BYTE_I32: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 704,
};

pub const X86_64_LOAD64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 700,
};

pub const X86_64_LOAD8_INDEXED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 703,
};

pub const X86_64_COPY_BYTES: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 736,
};

pub const X86_64_STORE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 705,
};

pub const X86_64_ADDRESS_OFFSET: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 706,
};

pub const X86_64_STORE64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 701,
};

pub const X86_64_FRAME_ADDRESS: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 702,
};

pub const X86_64_LINUX_SYSTEM_CALL: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::SystemCall,
    variant: 0,
};

pub const X86_64_INLINE_ASSEMBLY_DEFAULT: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::InlineAssembly,
    variant: 0,
};

/// Canonical Boolean materialization reads condition state and defines every GPR bit.
pub const X86_64_MATERIALIZE_BOOLEAN: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 732,
};

pub const X86_64_MATERIALIZE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 0,
};

pub const X86_64_COPY_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 1,
};

pub const X86_64_COMPARE_I64_ZERO: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 2,
};

pub const X86_64_CONDITIONAL_BRANCH: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 3,
};

/// Flag-transparent three-address i64 addition, realizable with an x86-64 LEA
/// form without introducing a false two-address tie.
pub const X86_64_ADD_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 4,
};

/// Flag-transparent `result = left + immediate`, realizable as LEA for the
/// named admitted immediate domain without a destructive two-address tie.
pub const X86_64_ADD_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 5,
};

/// Total unsigned subtraction of every unsigned carrier (a zero-normalized
/// narrow difference borrows exactly when the u64 one does) with an
/// early-clobber result and RFLAGS clobber.
pub const X86_64_SATURATING_SUBTRACT_UNSIGNED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 54,
};

/// Total unsigned addition clamps to the maximum u64 value.
pub const X86_64_SATURATING_ADD_U64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 55,
};

/// Unsigned division consumes an explicit zero RDX input and clobbers the remainder.
pub const X86_64_DIVIDE_U64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 56,
};

/// Signed wrapping remainder reads RAX, reads its divisor pinned to RCX so the
/// realized form may zero RDX for the MIN / -1 guard, and defines RAX plus an
/// ordinary late RDX scratch output.
pub const X86_64_REMAINDER_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 57,
};

/// Saturating addition of every carrier but u64: MOV/ADD into an
/// early-clobber result, clamped through an early-clobber bound scratch
/// (MOVABS/CMP/CMOV against the carrier bounds for narrow carriers, a CMOVO
/// overflow select for i64); clobbers RFLAGS.
pub const X86_64_SATURATING_ADD_CLAMPED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 58,
};

/// Signed saturating subtraction with the same early-clobber result and
/// bound scratch as the clamped addition.
pub const X86_64_SATURATING_SUBTRACT_CLAMPED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 59,
};

/// Signed saturating division: CQO/IDIV on RAX with the explicit RDX input
/// of unsigned division. Narrow carriers clamp the only out-of-range quotient
/// MIN / -1 through the carrier maximum held in RDX; the i64 carrier guards
/// that faulting dividend through RDX before dividing. RDX is clobbered.
pub const X86_64_SATURATING_DIVIDE_SIGNED: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 60,
};

/// Exact `result = left * right` three-address pseudo. Its realization must be
/// alias-safe for every allocator result: `IMUL result, right` when the result
/// aliases the left input, `IMUL result, left` when it aliases only the right
/// input, `IMUL result, result` reads the aliased input when all three share a
/// view, and `MOV; IMUL` otherwise. `IMUL` defines RFLAGS.
pub const X86_64_MULTIPLY_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 61,
};

/// Signed i64 wrapping division. `IDIV` faults on MIN / -1 while the wrapping
/// semantics require i64::MIN, so the realization guards that divisor: it
/// initializes the result to the wrapping quotient, compares the divisor
/// against -1, branches over the divide on equality, otherwise sign-extends
/// the dividend with CQO and executes IDIV. The dividend and quotient live in
/// RAX, RDX is the sign-extension clobber, and the compare defines RFLAGS.
pub const X86_64_DIVIDE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 62,
};

/// Unsigned u64 remainder on the fixed RAX:RDX pair: the dividend occupies
/// RAX, RDX is zeroed as the high input, DIV leaves the remainder in RDX, and
/// a move returns it to the result view. Both fixed registers are clobbered
/// and RFLAGS is undefined across the divide.
pub const X86_64_REMAINDER_U64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 63,
};

/// Exact `result = left - right` three-address pseudo. Its realization must be
/// alias-safe for every allocator result: `XOR result, result` when both inputs
/// share a view, `SUB` when the result is only the left input, `NEG; ADD` when
/// it is only the right input, and `MOV; SUB` otherwise. Those alternatives do
/// not preserve one common flags value, so the row explicitly clobbers RFLAGS.
pub const X86_64_SUBTRACT_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 6,
};

/// Flag-transparent `result = left - immediate`, realized as an x86-64 LEA
/// with the negated admitted U12 displacement and no destructive tie.
pub const X86_64_SUBTRACT_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 7,
};

/// Two-input i64 comparison. Both operands are read and RFLAGS is defined.
pub const X86_64_COMPARE_I64: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 8,
};

/// One-input i64 comparison against the encoded unsigned immediate. The
/// operand is read and RFLAGS is defined, matching `cmp r64, imm32`.
pub const X86_64_COMPARE_I64_IMMEDIATE: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 10,
};

/// Unconditional relative control without a condition-register dependency.
pub const X86_64_JUMP: RegisterConstraintKey = RegisterConstraintKey {
    family: RegisterConstraintFamily::Instruction,
    variant: 9,
};

/// Closed v1 inventory owned by the x86-64 target.
///
/// The ordinary rows are deliberately limited to the baseline operations
/// required by a register-passed scalar conditional-return CFG plus the first
/// arithmetic row needed by the pressure vertical. This is not a claim that
/// the target's ordinary instruction inventory is complete.
pub const X86_64_REQUIRED_REGISTER_CONSTRAINTS: [RegisterConstraintKey; 71] = [
    X86_64_SYSTEM_V_CALL,
    X86_64_MICROSOFT_CALL,
    X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64,
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
    X86_64_MICROSOFT_CALL_UNIT,
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
        variant: 723,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 724,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 740,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 741,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 742,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 743,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 744,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 745,
    },
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Call,
        variant: 746,
    },
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
];
