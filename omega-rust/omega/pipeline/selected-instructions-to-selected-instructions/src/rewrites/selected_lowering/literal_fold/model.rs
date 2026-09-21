use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, TargetRegisterEnvironmentIdentity};
pub use selected_instructions::LiteralFoldIdentity;
use selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::rewrites::selected_lowering::literal_fold::identity::encode_terminal_literal_fold_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    RecoveryClassificationIdentity, SpillChoiceIdentity,
};

const LITERAL_FOLD_MAGIC: &[u8; 8] = b"OMGLFD\0\0";
const LITERAL_FOLD_VERSION: u32 = 12;

/// Narrow proof-preserving physical-form fold. This is not a generic constant
/// fold, instruction scheduler, rematerializer, spill policy, or opt level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralFoldPolicy {
    enabled_rules: u32,
}

impl LiteralFoldPolicy {
    const EXACT_ADD_BIT: u32 = 1 << 0;
    const EXACT_SUBTRACT_BIT: u32 = 1 << 1;
    const COMPARE_BIT: u32 = 1 << 2;
    const EXTENSION_BIT: u32 = 1 << 3;
    const LOAD8_INDEXED_BIT: u32 = 1 << 4;
    const COPY_BIT: u32 = 1 << 5;
    const BYTE_VIEW_ADDRESS_BIT: u32 = 1 << 6;
    const EXACT_DIVIDE_BIT: u32 = 1 << 7;
    const WRAPPING_REMAINDER_BIT: u32 = 1 << 8;
    const BITWISE_AND_ZERO_BIT: u32 = 1 << 9;
    const BITWISE_XOR_ZERO_BIT: u32 = 1 << 10;
    const WRAPPING_ADD_ZERO_BIT: u32 = 1 << 11;
    const BITWISE_AND_ONES_BIT: u32 = 1 << 12;
    const WRAPPING_REMAINDER_ZERO_BIT: u32 = 1 << 13;
    const EXACT_DIVIDE_ZERO_BIT: u32 = 1 << 14;
    const SATURATING_ADD_ZERO_BIT: u32 = 1 << 15;
    const SATURATING_SUBTRACT_ZERO_BIT: u32 = 1 << 16;
    const SATURATING_DIVIDE_ONE_BIT: u32 = 1 << 17;
    const SATURATING_DIVIDE_ZERO_BIT: u32 = 1 << 18;
    const SATURATING_SUBTRACT_ZERO_MINUEND_BIT: u32 = 1 << 19;
    const SATURATING_ADD_UPPER_BOUND_BIT: u32 = 1 << 20;
    const WRAPPING_REMAINDER_MINUS_ONE_BIT: u32 = 1 << 21;
    const SATURATING_SUBTRACT_UPPER_BOUND_BIT: u32 = 1 << 22;
    const KNOWN_BITS: u32 = Self::EXACT_ADD_BIT
        | Self::EXACT_SUBTRACT_BIT
        | Self::COMPARE_BIT
        | Self::EXTENSION_BIT
        | Self::LOAD8_INDEXED_BIT
        | Self::COPY_BIT
        | Self::BYTE_VIEW_ADDRESS_BIT
        | Self::EXACT_DIVIDE_BIT
        | Self::WRAPPING_REMAINDER_BIT
        | Self::BITWISE_AND_ZERO_BIT
        | Self::BITWISE_XOR_ZERO_BIT
        | Self::WRAPPING_ADD_ZERO_BIT
        | Self::BITWISE_AND_ONES_BIT
        | Self::WRAPPING_REMAINDER_ZERO_BIT
        | Self::EXACT_DIVIDE_ZERO_BIT
        | Self::SATURATING_ADD_ZERO_BIT
        | Self::SATURATING_SUBTRACT_ZERO_BIT
        | Self::SATURATING_DIVIDE_ONE_BIT
        | Self::SATURATING_DIVIDE_ZERO_BIT
        | Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT
        | Self::SATURATING_ADD_UPPER_BOUND_BIT
        | Self::WRAPPING_REMAINDER_MINUS_ONE_BIT
        | Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT;

    pub const EXACT_ADD_V1: Self = Self {
        enabled_rules: Self::EXACT_ADD_BIT,
    };
    pub const EXACT_SUBTRACT_V1: Self = Self {
        enabled_rules: Self::EXACT_SUBTRACT_BIT,
    };
    pub const COMPARE_V1: Self = Self {
        enabled_rules: Self::COMPARE_BIT,
    };
    /// Exact unary extension elimination: fold a materialized incoming literal
    /// through its sole `ZeroExtend`/`SignExtend` consumer into a direct
    /// `MaterializeI64` of the extension's exact output bits.
    pub const EXTENSION_V1: Self = Self {
        enabled_rules: Self::EXTENSION_BIT,
    };
    /// Indexed byte-load offset materialization: fold a materialized incoming
    /// literal through its sole `Load8Indexed` consumer into the direct-offset
    /// `Load8` form the literal names.
    pub const LOAD8_INDEXED_V1: Self = Self {
        enabled_rules: Self::LOAD8_INDEXED_BIT,
    };
    /// Copy-constant materialization: fold a materialized incoming literal
    /// through its sole `CopyI64` consumer into a direct `MaterializeI64` of
    /// the literal at the copy's destination register.
    pub const COPY_V1: Self = Self {
        enabled_rules: Self::COPY_BIT,
    };
    /// Address-mode folding: fold a materialized incoming literal through its
    /// sole `ByteViewAddress` consumer into the constant-offset
    /// `AddressOffset` form the literal names. The projection computes
    /// `(backing + offset) modulo 2^64` — a commutative modular address
    /// addition — so the literal folds at either `Use` operand: the
    /// operand-1 offset literal and the operand-0 backing literal both
    /// rewrite into the same `AddressOffset` row, and the folded literal's
    /// operand position names which grammar the fold belongs to.
    pub const BYTE_VIEW_ADDRESS_V1: Self = Self {
        enabled_rules: Self::BYTE_VIEW_ADDRESS_BIT,
    };
    /// Exact-division identity: fold a materialized literal `1` feeding its
    /// sole `ExactDivideU64` consumer's divisor operand into a `CopyI64` of
    /// the dividend — a divide by one is the dividend itself, so the
    /// consumer's encoded fault surface is discharged by the literal.
    pub const EXACT_DIVIDE_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_BIT,
    };
    /// Wrapping-remainder constant fold: fold a materialized literal `1`
    /// feeding its sole `WrappingRemainderI64` consumer's divisor operand
    /// into a `MaterializeI64` of zero at the result register — a remainder
    /// by one is always zero, so the consumer's encoded fault surface is
    /// discharged by the literal and its scratch `Def` outputs drop dead.
    pub const WRAPPING_REMAINDER_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_BIT,
    };
    /// Bitwise-and annihilator fold: fold a materialized literal `0`
    /// feeding its sole `BitwiseAndI64` consumer at either `Use` operand
    /// into a `MaterializeI64` of zero at the result register — zero is
    /// the bitwise-and annihilator, so the constant result never reads
    /// the surviving-side `Use` the fold drops.
    pub const BITWISE_AND_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ZERO_BIT,
    };
    /// Bitwise-xor identity fold: fold a materialized literal `0` feeding
    /// its sole `BitwiseXorI64` consumer at either `Use` operand into a
    /// `CopyI64` of the other `Use` — zero is the bitwise-xor identity
    /// element, so `x ^ 0` and `0 ^ x` are both `x` and the surviving
    /// operand's register moves to the result unchanged.
    pub const BITWISE_XOR_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_XOR_ZERO_BIT,
    };
    /// Wrapping-add identity fold: fold a materialized literal `0` feeding
    /// its sole `WrappingAddI64` consumer at either `Use` operand into a
    /// `CopyI64` of the other `Use` — zero is the additive identity under
    /// modulo-2^64 wrap, so `x + 0` and `0 + x` are both `x` and the
    /// surviving operand's register moves to the result unchanged. The
    /// wrapping-add consumer binds the flag-transparent add row, so the
    /// fold carries no flag clobber to retire.
    pub const WRAPPING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_ADD_ZERO_BIT,
    };
    /// Bitwise-and identity fold: fold a materialized all-ones literal
    /// (`u64::MAX`) feeding its sole `BitwiseAndI64` consumer at either
    /// `Use` operand into a `CopyI64` of the other `Use` — all-ones is the
    /// bitwise-and identity element, so `x & MAX` and `MAX & x` are both
    /// `x` and the surviving operand's register moves to the result
    /// unchanged. The all-ones grammar is disjoint from the and-zero
    /// annihilator family on the literal's value: the producer selects
    /// between the two `BitwiseAndI64` families by which exact literal the
    /// enabled rules admit.
    pub const BITWISE_AND_ONES_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ONES_BIT,
    };
    /// Wrapping-remainder zero-dividend fold: fold a materialized literal
    /// `0` feeding its sole `WrappingRemainderI64` consumer's dividend
    /// operand into a `MaterializeI64` of zero at the result register — a
    /// remainder of a zero dividend is always zero. The literal is not
    /// what discharges the consumer's encoded fault surface: the
    /// nonzero-divisor obligation the remainder kind carries already
    /// excludes division by zero, and the zero dividend fixes a quotient
    /// that cannot overflow — so the fold retires the trap surface under
    /// the consumer's own definedness proof while dropping the divisor
    /// `Use` and dead scratch `Def` operands. The zero-dividend grammar
    /// is disjoint from the divisor-one family on the folded literal's
    /// operand position: the producer selects between the two
    /// `WrappingRemainderI64` families by which position the recorded
    /// future use names.
    pub const WRAPPING_REMAINDER_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_ZERO_BIT,
    };
    /// Exact-divide zero-dividend fold: fold a materialized literal `0`
    /// feeding its sole `ExactDivideU64` consumer's dividend operand into
    /// a `MaterializeI64` of zero at the result register — an unsigned
    /// divide of a zero dividend is always zero. The literal is not what
    /// discharges the consumer's encoded fault surface: a quotient of
    /// zero can never overflow, and the nonzero-divisor obligation the
    /// exact-divide kind carries already excludes division by zero — so
    /// the fold retires the trap surface under the consumer's own
    /// definedness proof while dropping the divisor `Use` and the
    /// provably-zero auxiliary `Use` operands a `div` realization reads
    /// as the dividend's upper half. The zero-dividend grammar is
    /// disjoint from the divisor-one family on the folded literal's
    /// operand position: the producer selects between the two
    /// `ExactDivideU64` families by which position the recorded future
    /// use names.
    pub const EXACT_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_ZERO_BIT,
    };
    /// Saturating-add identity fold: fold a materialized literal `0`
    /// feeding its sole `SaturatingAdd` consumer on any carrier at either
    /// `Use` operand into a `CopyI64` of the other `Use` — zero is the
    /// additive identity under saturating addition, so `x +| 0` and
    /// `0 +| x` are both `x` inside the carrier's bounds and the
    /// surviving operand's register moves to the result unchanged. The
    /// saturating-add consumer implicitly defines the target condition
    /// state on aarch64 — every carrier's realization is flag-setting —
    /// and clobbers `rflags` on x86-64; the fold retires both with the
    /// folded form, admitting the consumer only while every unit its
    /// record defines is dead in the function: a reader of a retired
    /// definition would observe a stale unit. The u64 carrier binds the
    /// three-operand row; every other carrier binds the clamped row
    /// whose operand list continues past the `Def` result with a bound
    /// scratch `Def` the fold drops under occurrence-free custody — a
    /// scratch output another instruction read or defined would leave a
    /// use of a register the rewrite stopped defining.
    pub const SATURATING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_ZERO_BIT,
    };
    /// Saturating-subtract identity fold: fold a materialized literal `0`
    /// feeding its sole `SaturatingSubtract` consumer on any carrier at
    /// the right `Use` operand into a `CopyI64` of the left `Use` — zero
    /// is the right identity under saturating subtraction, so `x -| 0`
    /// is `x` inside the carrier's bounds and the surviving operand's
    /// register moves to the result unchanged. The grammar is
    /// deliberately asymmetric: `0 -| x` is `-x` clamped to the carrier's
    /// bounds on a signed carrier and `0` on an unsigned one, not `x`,
    /// so this family declares no left-literal rule and a literal at
    /// operand 0 rejects under it — the unsigned `0 -| x` constant fold
    /// lives in the disjoint zero-minuend family below. The
    /// saturating-subtract consumer implicitly
    /// defines the target condition state on aarch64 — every carrier's
    /// realization is flag-setting — and clobbers `rflags` on x86-64;
    /// the fold retires both with the folded form, admitting the consumer
    /// only while every unit its record defines is dead in the function:
    /// a reader of a retired definition would observe a stale unit.
    /// Unsigned carriers bind the three-operand row; signed carriers bind
    /// the clamped row whose operand list continues past the `Def`
    /// result with a bound scratch `Def` the fold drops under
    /// occurrence-free custody — a scratch output another instruction
    /// read or defined would leave a use of a register the rewrite
    /// stopped defining.
    pub const SATURATING_SUBTRACT_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_ZERO_BIT,
    };
    /// Saturating-divide identity fold: fold a materialized literal `1`
    /// feeding its sole `SaturatingDivide` consumer's divisor operand on
    /// any carrier into a `CopyI64` of the dividend `Use` — a saturating
    /// divide by one is the dividend, so `x /| 1` is `x` inside the
    /// carrier's bounds and the surviving operand's register moves to the
    /// result unchanged. The grammar is deliberately asymmetric:
    /// `1 /| x` is not `x`, so no left-literal rule exists and a literal
    /// at operand 0 rejects. The consumer may architecturally fault —
    /// the x86-64 `div`/`idiv` realization encodes divide by zero and
    /// quotient overflow — and the folded divisor of one is itself the
    /// discharging evidence: a divide by one can do neither, so the
    /// rewrite retires the trap surface wholesale. The signed-carrier
    /// aarch64 consumer also implicitly defines `nzcv`; the fold retires
    /// the definition with the folded form, admitting the consumer only
    /// while every unit its record defines is dead in the function. The
    /// consumer's operand list may continue past the `Def` result under
    /// the mixed-tail grammar: a `Use` — the zeroed high-half dividend an
    /// x86-64 `div`/`idiv` reads — drops only when defined solely by zero
    /// materializations, and a `Def` — the bound scratch an aarch64
    /// signed row writes — drops only when its register occurs nowhere
    /// else in the function.
    pub const SATURATING_DIVIDE_ONE_V1: Self = Self {
        enabled_rules: Self::SATURATING_DIVIDE_ONE_BIT,
    };
    /// Saturating-divide zero-dividend fold: fold a materialized literal
    /// `0` feeding its sole `SaturatingDivide` consumer's dividend
    /// operand on any carrier into a `MaterializeI64` of zero at the
    /// result register — a saturating divide of a zero dividend is
    /// always zero inside the carrier's bounds. The literal is not what
    /// discharges the consumer's encoded fault surface: a quotient of
    /// zero can never overflow or clamp, and the nonzero-divisor
    /// obligation the saturating-divide kind carries already excludes
    /// division by zero — so the fold retires the trap surface under the
    /// consumer's own definedness proof while dropping the divisor `Use`
    /// and every tail operand under its own custody: the provably-zero
    /// auxiliary `Use` an x86-64 `div`/`idiv` realization reads as the
    /// dividend's upper half, and the bound scratch `Def` an aarch64
    /// clamped signed row writes. The signed-carrier aarch64 consumer
    /// also implicitly defines `nzcv`; the fold retires the definition
    /// with the folded form, admitting the consumer only while every
    /// unit its record defines is dead in the function. The
    /// zero-dividend grammar is disjoint from the divisor-one family on
    /// the folded literal's operand position: the producer selects
    /// between the two `SaturatingDivide` families by which position the
    /// recorded future use names.
    pub const SATURATING_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_DIVIDE_ZERO_BIT,
    };
    /// Saturating-subtract zero-minuend fold: fold a materialized literal
    /// `0` feeding its sole `SaturatingSubtract` consumer's minuend
    /// operand on an unsigned carrier into a `MaterializeI64` of zero at
    /// the result register — `0 -| x` is `0` for every `x` because the
    /// subtraction underflows the carrier's lower bound and saturates to
    /// it. The constant result never reads the operand-1 subtrahend
    /// `Use`, so the fold drops it with the form, and retires the
    /// consumer's implicit unit surface under the same deadness gate the
    /// identity family carries: the aarch64 realization's `nzcv`
    /// definition may retire only while no instruction or terminator in
    /// the function implicitly uses it, and the x86-64 row's `rflags`
    /// clobber retires unconditionally. Signed carriers admit no
    /// operand-0 fold at all — `0 -| x` there is `-x` clamped to the
    /// carrier's bounds, not a constant — so the family binds only the
    /// unsigned three-operand row. The zero-minuend grammar is disjoint
    /// from the right-zero identity family on the folded literal's
    /// operand position: the producer selects between the two
    /// `SaturatingSubtract` families by which position the recorded
    /// future use names.
    pub const SATURATING_SUBTRACT_ZERO_MINUEND_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT,
    };
    /// Saturating-add upper-bound fold: fold a materialized literal equal
    /// to the carrier's maximum feeding its sole `SaturatingAdd` consumer
    /// on an unsigned carrier at either `Use` operand into a
    /// `MaterializeI64` of that maximum at the result register —
    /// `x +| MAX` and `MAX +| x` are both `MAX` for every `x` an unsigned
    /// carrier admits, because `x + MAX` reaches the carrier's upper
    /// bound and saturates to it. Signed carriers admit no maximum fold
    /// at all: `x +| MAX` there is `x + MAX` unclamped for every negative
    /// `x`, not a constant, so the family binds no signed pair. The
    /// constant result never reads the surviving-side `Use` the fold
    /// drops, and every operand past the operand-2 `Def` result — the
    /// clamped row's bound scratch `Def` — drops under occurrence-free
    /// custody. The consumer retires the same target-specific unit
    /// effects the identity family does — aarch64 defines `nzcv`,
    /// x86-64 clobbers `rflags` — admitted only while every unit its
    /// record defines is dead in the function. The u64 carrier binds the
    /// three-operand row; every other unsigned carrier binds the clamped
    /// row. The family shares its consumer kind and operand positions
    /// with the zero-identity fold: the literal's value names which
    /// family a `SaturatingAdd` fold belongs to, and admission requires
    /// exactly one enabled pair to admit the recorded immediate.
    pub const SATURATING_ADD_UPPER_BOUND_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_UPPER_BOUND_BIT,
    };
    /// Wrapping-remainder minus-one fold: fold a materialized literal
    /// `u64::MAX` — the normalized-i64 divisor `-1` — feeding its sole
    /// `WrappingRemainderI64` consumer's divisor operand into a
    /// `MaterializeI64` of zero at the result register — a remainder by
    /// minus one is always zero: `x % -1` is `0` for every `x`, and
    /// `i64::MIN % -1` is the exceptional case the kind's semantics
    /// defines to produce zero rather than trap. The literal is itself
    /// the evidence the consumer's encoded fault surface cannot fire —
    /// a divisor of `-1` can never divide by zero, and the one dividend
    /// whose `idiv` would overflow is the case the realization's `-1`
    /// guard skips — so the fold retires the trap surface wholesale
    /// while dropping the dividend `Use` and dead scratch `Def`
    /// operands. The minus-one grammar is disjoint from the divisor-one
    /// family on the folded literal's value at the same operand
    /// position: the producer selects between the two
    /// `WrappingRemainderI64` divisor families by which exact literal
    /// the enabled rules admit.
    pub const WRAPPING_REMAINDER_MINUS_ONE_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_MINUS_ONE_BIT,
    };
    /// Saturating-subtract upper-bound subtrahend fold: fold a
    /// materialized literal equal to the carrier's maximum feeding its
    /// sole `SaturatingSubtract` consumer's operand-1 subtrahend `Use` on
    /// an unsigned carrier into a `MaterializeI64` of zero at the result
    /// register — `x -| MAX` is `0` for every `x` the carrier admits,
    /// because `x - MAX` never exceeds zero and underflows the carrier's
    /// lower bound — saturating to it — for every `x < MAX`, and is
    /// exactly zero at `x == MAX`. Only unsigned carriers admit the
    /// fold: under signed saturation `x -| MAX` is `x - MAX` clamped to
    /// the carrier's lower bound for every negative `x`, not a constant,
    /// so the family binds no signed pair. The constant result never
    /// reads the operand-0 minuend `Use`, so the fold drops it with the
    /// form, and retires the consumer's implicit unit surface under the
    /// same deadness gate the sibling families carry: the aarch64
    /// realization's `nzcv` definition may retire only while no
    /// instruction or terminator in the function implicitly uses it, and
    /// the x86-64 row's `rflags` clobber retires unconditionally. The
    /// family shares its consumer kind and operand position with the
    /// right-zero identity fold: the folded literal's value names which
    /// `SaturatingSubtract` subtrahend family a fold belongs to, and it
    /// stays position-disjoint from the unsigned zero-minuend family at
    /// operand 0 — `MAX -| x` is `MAX - x`, not a constant, so the
    /// family declares no left-literal pair.
    pub const SATURATING_SUBTRACT_UPPER_BOUND_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT,
    };

    pub(crate) const fn empty() -> Self {
        Self { enabled_rules: 0 }
    }

    pub const fn contains(self, other: Self) -> bool {
        other.enabled_rules != 0 && self.enabled_rules & other.enabled_rules == other.enabled_rules
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            enabled_rules: self.enabled_rules | other.enabled_rules,
        }
    }

    pub const fn enables_exact_add(self) -> bool {
        self.enabled_rules & Self::EXACT_ADD_BIT != 0
    }

    pub const fn enables_exact_subtract(self) -> bool {
        self.enabled_rules & Self::EXACT_SUBTRACT_BIT != 0
    }

    pub const fn enables_compare(self) -> bool {
        self.enabled_rules & Self::COMPARE_BIT != 0
    }

    pub const fn enables_extension(self) -> bool {
        self.enabled_rules & Self::EXTENSION_BIT != 0
    }

    pub const fn enables_load8_indexed(self) -> bool {
        self.enabled_rules & Self::LOAD8_INDEXED_BIT != 0
    }

    pub const fn enables_copy(self) -> bool {
        self.enabled_rules & Self::COPY_BIT != 0
    }

    pub const fn enables_byte_view_address(self) -> bool {
        self.enabled_rules & Self::BYTE_VIEW_ADDRESS_BIT != 0
    }

    pub const fn enables_exact_divide(self) -> bool {
        self.enabled_rules & Self::EXACT_DIVIDE_BIT != 0
    }

    pub const fn enables_wrapping_remainder(self) -> bool {
        self.enabled_rules & Self::WRAPPING_REMAINDER_BIT != 0
    }

    pub const fn enables_bitwise_and_zero(self) -> bool {
        self.enabled_rules & Self::BITWISE_AND_ZERO_BIT != 0
    }

    pub const fn enables_bitwise_xor_zero(self) -> bool {
        self.enabled_rules & Self::BITWISE_XOR_ZERO_BIT != 0
    }

    pub const fn enables_wrapping_add_zero(self) -> bool {
        self.enabled_rules & Self::WRAPPING_ADD_ZERO_BIT != 0
    }

    pub const fn enables_bitwise_and_ones(self) -> bool {
        self.enabled_rules & Self::BITWISE_AND_ONES_BIT != 0
    }

    pub const fn enables_wrapping_remainder_zero(self) -> bool {
        self.enabled_rules & Self::WRAPPING_REMAINDER_ZERO_BIT != 0
    }

    pub const fn enables_exact_divide_zero(self) -> bool {
        self.enabled_rules & Self::EXACT_DIVIDE_ZERO_BIT != 0
    }

    pub const fn enables_saturating_add_zero(self) -> bool {
        self.enabled_rules & Self::SATURATING_ADD_ZERO_BIT != 0
    }

    pub const fn enables_saturating_subtract_zero(self) -> bool {
        self.enabled_rules & Self::SATURATING_SUBTRACT_ZERO_BIT != 0
    }

    pub const fn enables_saturating_divide_one(self) -> bool {
        self.enabled_rules & Self::SATURATING_DIVIDE_ONE_BIT != 0
    }

    pub const fn enables_saturating_divide_zero(self) -> bool {
        self.enabled_rules & Self::SATURATING_DIVIDE_ZERO_BIT != 0
    }

    pub const fn enables_saturating_subtract_zero_minuend(self) -> bool {
        self.enabled_rules & Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT != 0
    }

    pub const fn enables_saturating_add_upper_bound(self) -> bool {
        self.enabled_rules & Self::SATURATING_ADD_UPPER_BOUND_BIT != 0
    }

    pub const fn enables_wrapping_remainder_minus_one(self) -> bool {
        self.enabled_rules & Self::WRAPPING_REMAINDER_MINUS_ONE_BIT != 0
    }

    pub const fn enables_saturating_subtract_upper_bound(self) -> bool {
        self.enabled_rules & Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT != 0
    }

    pub const fn canonical_bits(self) -> u32 {
        self.enabled_rules
    }

    pub const fn from_canonical_bits(bits: u32) -> Option<Self> {
        if bits == 0 || bits & !Self::KNOWN_BITS != 0 {
            None
        } else {
            Some(Self {
                enabled_rules: bits,
            })
        }
    }
}

/// Canonical recipe and output commitment. The transformed selected CFG stays
/// private to the validated carrier and is independently reconstructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFoldPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    /// The validated machine-effect catalog the pair descriptors were
    /// admitted against: producer, consumer, and rewritten declarations are
    /// resolved in this catalog by both the producer and the independent
    /// replay.
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: LiteralFoldPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionLiteralFold>,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl LiteralFoldPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_literal_fold_content(self);
        let identity = crate::literal_fold_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(LITERAL_FOLD_MAGIC);
        encoded.extend_from_slice(&LITERAL_FOLD_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, LiteralFoldDecodeError> {
        let mut cursor = LiteralFoldCursor::new(encoded);
        if cursor.take(LITERAL_FOLD_MAGIC.len())? != LITERAL_FOLD_MAGIC {
            return Err(LiteralFoldDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != LITERAL_FOLD_VERSION {
            return Err(LiteralFoldDecodeError::UnsupportedVersion(version));
        }
        let identity = LiteralFoldIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel)
            .ok_or(LiteralFoldDecodeError::InvalidFuelSchedule(raw_fuel))?;
        let policy_bits = u32::from_le_bytes(cursor.array()?);
        let policy = LiteralFoldPolicy::from_canonical_bits(policy_bits)
            .ok_or(LiteralFoldDecodeError::UnknownPolicy(policy_bits))?;
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine)
                .ok_or(LiteralFoldDecodeError::InvalidMachineId(raw_machine))?;
            let action = match cursor.byte()? {
                0 => None,
                1 => Some(LiteralFoldAction {
                    block: SelectedBlockId(u32::from_le_bytes(cursor.array()?)),
                    pressure_point: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                    literal_instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
                    victim: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    consumer_instruction: SelectedInstructionId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    surviving: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    result: match cursor.byte()? {
                        0 => None,
                        1 => Some(VirtualRegisterId(u32::from_le_bytes(cursor.array()?))),
                        tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
                    },
                    immediate: u64::from_le_bytes(cursor.array()?),
                    immediate_constraint: decode_constraint_key(&mut cursor)?,
                }),
                tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
            };
            functions.push(FunctionLiteralFold { machine, action });
        }
        let transformed_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(LiteralFoldDecodeError::TrailingBytes);
        }
        let plan = Self {
            source_selected,
            spill_choices,
            recovery_classifications,
            ranges,
            legality,
            register_environment,
            allocator_availability,
            machine_effect_catalog,
            optimization_unit,
            fuel_schedule,
            policy,
            budget,
            usage,
            functions,
            transformed_selected,
        };
        if crate::literal_fold_identity(&plan) != identity {
            return Err(LiteralFoldDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiteralFold {
    pub machine: MachineId,
    pub action: Option<LiteralFoldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldAction {
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub literal_instruction: SelectedInstructionId,
    pub victim: VirtualRegisterId,
    pub consumer_instruction: SelectedInstructionId,
    /// The source register every `Use` position of the rewritten constraint
    /// row binds — the operand that survives the fold. Under a right-literal
    /// grammar it is the consumer's operand 0, under a left-literal grammar
    /// operand 1, under the `Use`-free unary grammar it records the folded
    /// input register, and under the constant-result grammars it records
    /// the dropped non-victim `Use` — the operand-0 dividend under the
    /// right grammar, the operand-1 `Use` under the left annihilator and
    /// left auxiliary-`Use` grammars — for custody; the rewritten row
    /// binds no `Use` position at all.
    pub surviving: VirtualRegisterId,
    /// The folded consumer's scalar result. Flag-defining consumers such as
    /// `CompareI64` carry no `Def` operand and record `None`.
    pub result: Option<VirtualRegisterId>,
    pub immediate: u64,
    pub immediate_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldValidationReceipt {
    pub(crate) identity: LiteralFoldIdentity,
    pub(crate) source_selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) recovery_classifications: RecoveryClassificationIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) machine_effect_catalog: MachineEffectCatalogIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
    pub(crate) policy: LiteralFoldPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
}

impl LiteralFoldValidationReceipt {
    pub const fn identity(self) -> LiteralFoldIdentity {
        self.identity
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
    }
    pub const fn recovery_classifications(self) -> RecoveryClassificationIdentity {
        self.recovery_classifications
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    /// The validated machine-effect catalog identity the fold's producer and
    /// independent replay both bound.
    pub const fn machine_effect_catalog(self) -> MachineEffectCatalogIdentity {
        self.machine_effect_catalog
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiteralFold {
    pub(crate) plan: LiteralFoldPlan,
    pub(crate) transformed: std::sync::Arc<SelectedInstructionPlan>,
    pub(crate) receipt: LiteralFoldValidationReceipt,
}

impl ValidatedLiteralFold {
    pub const fn plan(&self) -> &LiteralFoldPlan {
        &self.plan
    }
    /// Share current immutable data; the returned artifact grants no new authority.
    pub fn shared_transformed(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.transformed)
    }

    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }
    pub const fn receipt(&self) -> LiteralFoldValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralFoldError {
    RootMismatch,
    UnsupportedPolicy,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    ClassificationNotAdmitted {
        function: usize,
    },
    UnsupportedVictimRole {
        function: usize,
    },
    UnsupportedImmediate {
        function: usize,
    },
    FutureUseMismatch {
        function: usize,
    },
    LiteralMismatch {
        function: usize,
    },
    ConsumerMismatch {
        function: usize,
    },
    /// The producer, consumer, or rewritten catalog declaration does not
    /// satisfy the pair's declared machine-effect surface.
    EffectSurfaceMismatch {
        function: usize,
    },
    /// The bound machine-effect catalog does not declare a policy-enabled
    /// rewritten form under its constraint key, or that declaration violates
    /// the pair's declared effect isolation.
    EffectCatalogMismatch,
    ImmediateConstraintMismatch,
    IdentifierUnderflow {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    UsageMismatch,
    TransformedPlanMismatch,
    TransformedIdentityMismatch,
}

impl std::fmt::Display for LiteralFoldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "terminal literal fold failed: {self:?}")
    }
}

impl std::error::Error for LiteralFoldError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralFoldDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u32),
    UnknownOption(u8),
    UnknownConstraintFamily(u8),
    InvalidFuelSchedule(u32),
    InvalidMachineId(u64),
    InvalidBudget,
    InvalidUsage,
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}

impl std::fmt::Display for LiteralFoldDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid terminal literal fold: {self:?}")
    }
}

impl std::error::Error for LiteralFoldDecodeError {}

fn decode_constraint_key(
    cursor: &mut LiteralFoldCursor<'_>,
) -> Result<RegisterConstraintKey, LiteralFoldDecodeError> {
    let family = match cursor.byte()? {
        0 => register_model::RegisterConstraintFamily::Call,
        1 => register_model::RegisterConstraintFamily::Return,
        2 => register_model::RegisterConstraintFamily::SystemCall,
        3 => register_model::RegisterConstraintFamily::InlineAssembly,
        4 => register_model::RegisterConstraintFamily::Instruction,
        tag => return Err(LiteralFoldDecodeError::UnknownConstraintFamily(tag)),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: u32::from_le_bytes(cursor.array()?),
    })
}

struct LiteralFoldCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> LiteralFoldCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], LiteralFoldDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(LiteralFoldDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(LiteralFoldDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], LiteralFoldDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| LiteralFoldDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, LiteralFoldDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, LiteralFoldDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| LiteralFoldDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}
