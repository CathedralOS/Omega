//! The x86-64 realization shape of each Trapping form.
//!
//! A Trapping form computes the exact result of its primitive and executes
//! the `Crash` leaf's `ud2` in place when the settled policy predicate holds.
//! Every check is a short conditional jump that skips the two `ud2` bytes
//! exactly when the predicate is false, so the form either falls through
//! with the result or stops at the trap; no other control transfer leaves
//! it, and no instruction in it raises an architectural fault.
//!
//! The scalar transport keeps narrow (8/16/32-bit) carriers sign- or
//! zero-normalized in 64-bit registers. The 64-bit sum, difference, or
//! product of two normalized narrow operands, and the left shift of one by a
//! count below its width, is therefore exact, and only the range check
//! depends on the carrier: a signed result lies in its carrier exactly when
//! it survives sign extension from the carrier width (`movsx`/`movsxd` into
//! the scratch, then `cmp`), and an unsigned result exactly when no bit at or
//! above the width is set (`shr` of a scratch copy by the width sets ZF). A
//! u32 product may pass i64::MAX, but its low 64 bits are the exact unsigned
//! product and the shift test reads them unsigned; a negative unsigned
//! difference has its high bits set and traps the same way.
//!
//! The 64-bit carriers have no wider register for the exact result, so they
//! read the flag the arithmetic defines: OF for the i64 add, subtract, and
//! IMUL, CF (the carry out or the borrow) for the u64 add and subtract, and
//! the OF of MUL, set exactly when the RDX high half is nonzero, for the u64
//! product. MUL is the one baseline instruction producing the unsigned high
//! half, which is why the u64 multiply sits on division's fixed RAX:RDX row.
//!
//! Division checks before it divides, because x86 raises #DE on a zero
//! divisor and on a quotient the register cannot hold, and the process must
//! stop at the `ud2` instead. `test` traps a zero divisor at every carrier;
//! the signed carriers then compare the divisor with -1 and, only then, RAX
//! with the carrier minimum, trapping MIN / -1 and MIN % -1 (whose remainder
//! is zero, but which Trapping rejects). Normalized narrow operands cannot
//! otherwise fault in the 64-bit divide. i64::MIN does not fit a
//! sign-extended imm32, so the i64 guard compares RAX with 1 instead: only
//! i64::MIN - 1 overflows.
//!
//! Shifts compare the RCX count unsigned with the carrier width before any
//! shift: a negative count of any signed type is normalized to a value of at
//! least 2^63, so one `jb` decides the whole `0..width` predicate. A narrow
//! left shift is then range-checked like narrow arithmetic; a 64-bit left
//! shift is exact exactly when shifting the result back (`sar` for i64, `shr`
//! for u64) restores the value. A right shift by an in-range count is always
//! exact and stays normalized.
//!
//! A conversion is exact when the normalized source value lies in the
//! destination carrier; normalization makes the source register that value
//! whatever the source width, so only the source sign matters. A signed
//! source enters a narrower signed carrier exactly when it survives sign
//! extension from the carrier width. Every other non-identity pair is a test
//! that no bit at or above a boundary is set: the carrier width for an
//! unsigned carrier, the width minus one for a signed carrier (which also
//! rejects a huge unsigned source), and 63 for a signed source into u64
//! (its sign). A signed source into i64 and an unsigned source into u64 are
//! plain copies. The check runs on the scratch before the result is written.
//!
//! The encoder, the decoded-form validator, the footprint, and the
//! machine-effect catalog all read the shape from here so their operand
//! counts, sizes, and effects agree.

use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedTrapBehavior,
    SaturatingCarrier, SelectedInstructionKind, TrappingForm, TrappingOperation,
};
use semantic_vocabulary::IntegerSign;

/// The x86-64 constraint row a Trapping form is allocated against, which
/// fixes its operand layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrappingRow {
    /// `[left, right, result, scratch]`, both outputs early-clobber.
    Binary,
    /// `[rax, right, rax, rdx]` with the right operand outside RDX.
    FixedPair,
    /// `[value, rcx count, result, scratch]`, both outputs early-clobber.
    Shift,
    /// `[operand, result, scratch]`, both outputs early-clobber.
    Convert,
}

/// The two-operand arithmetic of an add, subtract, or multiply form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrappingArithmetic {
    /// `add result, right`.
    Add,
    /// `sub result, right`.
    Subtract,
    /// `imul result, right`.
    Multiply,
}

impl TrappingArithmetic {
    /// ADD and SUB r/m64, r64 are three bytes; IMUL r64, r/m64 is four.
    const fn byte_count(self) -> u16 {
        match self {
            Self::Add | Self::Subtract => 3,
            Self::Multiply => 4,
        }
    }
}

/// How one exact 64-bit value is tested through the scratch; the `je` over
/// the trap is taken exactly when the value is in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RangeCheck {
    /// `movsx scratch, value8|value16` or `movsxd scratch, value32`, then
    /// `cmp scratch, value; je; ud2`: the value lies in the signed carrier of
    /// `bits` exactly when sign extension from `bits` restores it.
    SignExtension { bits: u16 },
    /// `mov scratch, value; shr scratch, shift; je; ud2`: the value, read
    /// unsigned, is below `2^shift` exactly when the shift leaves zero.
    HighBits { shift: u8 },
}

impl RangeCheck {
    /// The check that an exact 64-bit value lies in a narrow carrier.
    const fn of_narrow_carrier(carrier: SaturatingCarrier) -> Self {
        if carrier.is_signed() {
            Self::SignExtension {
                bits: carrier.bits(),
            }
        } else {
            Self::HighBits {
                shift: carrier.bits() as u8,
            }
        }
    }

    /// The check that a normalized source of `source` sign lies in the
    /// destination carrier, or `None` when every such value does.
    const fn of_conversion(source: IntegerSign, carrier: SaturatingCarrier) -> Option<Self> {
        match (source, carrier) {
            (IntegerSign::Signed, SaturatingCarrier::I64)
            | (IntegerSign::Unsigned, SaturatingCarrier::U64) => None,
            (IntegerSign::Signed, SaturatingCarrier::U64) => Some(Self::HighBits { shift: 63 }),
            (IntegerSign::Signed, _) if carrier.is_signed() => Some(Self::SignExtension {
                bits: carrier.bits(),
            }),
            _ if carrier.is_signed() => Some(Self::HighBits {
                shift: carrier.bits() as u8 - 1,
            }),
            _ => Some(Self::HighBits {
                shift: carrier.bits() as u8,
            }),
        }
    }

    pub(crate) const fn byte_count(self) -> u16 {
        match self {
            // MOVSXD (3), CMP (3), JE (2), UD2 (2).
            Self::SignExtension { bits: 32 } => 10,
            // MOVSX (4), CMP (3), JE (2), UD2 (2).
            Self::SignExtension { .. } => 11,
            // MOV (3), SHR imm8 (4), JE (2), UD2 (2).
            Self::HighBits { .. } => 11,
        }
    }
}

/// How a signed division traps the carrier-minimum dividend once the divisor
/// is known to be -1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DividendGuard {
    /// `cmp rax, imm8; jne; ud2`: i8::MIN fits the sign-extended imm8.
    Minimum8(i8),
    /// `cmp rax, imm32; jne; ud2` in the RAX short form (`48 3d`): the i16
    /// and i32 minimums.
    Minimum32(i32),
    /// `cmp rax, 1; jno; ud2`: i64::MIN is the one dividend whose decrement
    /// overflows.
    DecrementOverflows,
}

impl DividendGuard {
    const fn of(carrier: SaturatingCarrier) -> Option<Self> {
        match carrier {
            SaturatingCarrier::I8 => Some(Self::Minimum8(i8::MIN)),
            SaturatingCarrier::I16 => Some(Self::Minimum32(i16::MIN as i32)),
            SaturatingCarrier::I32 => Some(Self::Minimum32(i32::MIN)),
            SaturatingCarrier::I64 => Some(Self::DecrementOverflows),
            SaturatingCarrier::U8
            | SaturatingCarrier::U16
            | SaturatingCarrier::U32
            | SaturatingCarrier::U64 => None,
        }
    }

    /// The compare, its conditional jump, and the UD2 that the preceding
    /// `jne` over a divisor other than -1 skips.
    pub(crate) const fn byte_count(self) -> u16 {
        match self {
            // CMP RAX, imm8 (4), JNE or JNO (2), UD2 (2).
            Self::Minimum8(_) | Self::DecrementOverflows => 8,
            // CMP RAX, imm32 (6), JNE (2), UD2 (2).
            Self::Minimum32(_) => 10,
        }
    }
}

/// The realization class of a Trapping form. Forms of one class differ only
/// in the carrier facts the class carries, so two forms with equal shapes
/// emit identical bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrappingShape {
    /// i64 add, subtract, and multiply: `mov; add|sub|imul; jno; ud2` (10
    /// bytes, 11 with IMUL).
    OverflowFlag(TrappingArithmetic),
    /// u64 add and subtract: `mov; add|sub; jae; ud2` (10 bytes), CF being
    /// the carry out or the borrow.
    CarryFlag(TrappingArithmetic),
    /// Narrow add, subtract, and multiply: `mov; add|sub|imul` of the exact
    /// 64-bit result, then the carrier range check of it (16 to 18 bytes).
    NarrowRange(TrappingArithmetic, RangeCheck),
    /// u64 multiply: `mul right; jno; ud2` on the fixed RAX:RDX pair (7
    /// bytes).
    MultiplyHighHalf,
    /// Divide and remainder: `test right; jne; ud2`, then `xor rdx; div`
    /// (unsigned) or `cmp right, -1; jne` over the dividend guard and `cqo;
    /// idiv` (signed), then `mov rax, rdx` for a remainder (13 to 31 bytes).
    Division {
        guard: Option<DividendGuard>,
        remainder: bool,
    },
    /// i64 and u64 left shift: the count check, `mov; shl`, then the
    /// round trip `mov; sar|shr; cmp value; je; ud2` (27 bytes).
    ShiftLeftRoundTrip { signed: bool },
    /// Narrow left shift: the count check, `mov; shl`, then the carrier
    /// range check of the exact result (24 or 25 bytes).
    ShiftLeftRange { width: u8, check: RangeCheck },
    /// Right shift: the count check, then `mov; sar|shr` (14 bytes).
    ShiftRight { width: u8, signed: bool },
    /// Conversion: the operand's range check, if any, then `mov result,
    /// operand` (3, 13, or 14 bytes).
    Convert(Option<RangeCheck>),
}

impl TrappingShape {
    pub(crate) const fn of(form: TrappingForm) -> Self {
        let carrier = form.carrier;
        let arithmetic = match form.operation {
            TrappingOperation::Add => TrappingArithmetic::Add,
            TrappingOperation::Subtract => TrappingArithmetic::Subtract,
            _ => TrappingArithmetic::Multiply,
        };
        match form.operation {
            TrappingOperation::Multiply if matches!(carrier, SaturatingCarrier::U64) => {
                Self::MultiplyHighHalf
            }
            TrappingOperation::Add | TrappingOperation::Subtract | TrappingOperation::Multiply => {
                match carrier {
                    SaturatingCarrier::I64 => Self::OverflowFlag(arithmetic),
                    SaturatingCarrier::U64 => Self::CarryFlag(arithmetic),
                    _ => Self::NarrowRange(arithmetic, RangeCheck::of_narrow_carrier(carrier)),
                }
            }
            TrappingOperation::Divide | TrappingOperation::Remainder => Self::Division {
                guard: DividendGuard::of(carrier),
                remainder: matches!(form.operation, TrappingOperation::Remainder),
            },
            TrappingOperation::ShiftLeft if carrier.is_narrow() => Self::ShiftLeftRange {
                width: carrier.bits() as u8,
                check: RangeCheck::of_narrow_carrier(carrier),
            },
            TrappingOperation::ShiftLeft => Self::ShiftLeftRoundTrip {
                signed: carrier.is_signed(),
            },
            TrappingOperation::ShiftRight => Self::ShiftRight {
                width: carrier.bits() as u8,
                signed: carrier.is_signed(),
            },
            TrappingOperation::Convert { source } => {
                Self::Convert(RangeCheck::of_conversion(source, carrier))
            }
        }
    }

    pub(crate) const fn row(self) -> TrappingRow {
        match self {
            Self::OverflowFlag(_) | Self::CarryFlag(_) | Self::NarrowRange(..) => {
                TrappingRow::Binary
            }
            Self::MultiplyHighHalf | Self::Division { .. } => TrappingRow::FixedPair,
            Self::ShiftLeftRoundTrip { .. }
            | Self::ShiftLeftRange { .. }
            | Self::ShiftRight { .. } => TrappingRow::Shift,
            Self::Convert(_) => TrappingRow::Convert,
        }
    }

    pub(crate) const fn byte_count(self) -> u16 {
        // `jcc +2; ud2` after a flag-setting instruction.
        const TRAP_UNLESS: u16 = 4;
        // `cmp rcx, width; jb +2; ud2` before any shift.
        const COUNT_CHECK: u16 = 4 + TRAP_UNLESS;
        // `mov result, value; shl|sar|shr result, cl`.
        const SHIFT: u16 = 6;
        match self {
            // MOV (3), the arithmetic, then the flag's JNO or JAE over UD2.
            Self::OverflowFlag(arithmetic) | Self::CarryFlag(arithmetic) => {
                3 + arithmetic.byte_count() + TRAP_UNLESS
            }
            Self::NarrowRange(arithmetic, check) => {
                3 + arithmetic.byte_count() + check.byte_count()
            }
            // MUL (3), JNO over UD2.
            Self::MultiplyHighHalf => 3 + TRAP_UNLESS,
            Self::Division { guard, remainder } => {
                // TEST (3) and JNE over UD2 at every carrier.
                let zero = 3 + TRAP_UNLESS;
                let divide = match guard {
                    // XOR (3), DIV (3).
                    None => 6,
                    // CMP divisor, -1 (4), JNE (2), the guard, CQO (2), IDIV (3).
                    Some(guard) => 4 + 2 + guard.byte_count() + 5,
                };
                // MOV RAX, RDX.
                zero + divide + if remainder { 3 } else { 0 }
            }
            // MOV (3), SAR or SHR by CL (3), CMP (3), JE over UD2.
            Self::ShiftLeftRoundTrip { .. } => COUNT_CHECK + SHIFT + 9 + TRAP_UNLESS,
            Self::ShiftLeftRange { check, .. } => COUNT_CHECK + SHIFT + check.byte_count(),
            Self::ShiftRight { .. } => COUNT_CHECK + SHIFT,
            // MOV result, operand after the check.
            Self::Convert(None) => 3,
            Self::Convert(Some(check)) => check.byte_count() + 3,
        }
    }

    pub(crate) const fn operand_count(self) -> usize {
        match self.row() {
            TrappingRow::Binary | TrappingRow::FixedPair | TrappingRow::Shift => 4,
            TrappingRow::Convert => 3,
        }
    }

    /// Whether resolved register codes satisfy the row's pins: the fixed pair
    /// sits on RAX with its right operand outside RDX (which XOR, CQO, or MUL
    /// overwrites before or while the right operand is read), the shift count
    /// is RCX for CL, and every early-clobber output is written while the
    /// inputs are live, so it may alias neither an input nor the other output.
    pub(crate) fn accepts_registers(self, registers: &[u8]) -> bool {
        if registers.len() != self.operand_count() {
            return false;
        }
        let (inputs, outputs) = registers.split_at(registers.len() - 2);
        match self.row() {
            TrappingRow::FixedPair => {
                inputs[0] == 0 && outputs[0] == 0 && outputs[1] == 2 && inputs[1] != 2
            }
            TrappingRow::Shift if inputs[1] != 1 => false,
            TrappingRow::Binary | TrappingRow::Shift | TrappingRow::Convert => {
                outputs.iter().all(|output| !inputs.contains(output)) && outputs[0] != outputs[1]
            }
        }
    }

    /// Operand positions read and written: every row's uses and definitions,
    /// as its constraint row states them. The scratch (or RDX) is a declared
    /// write even for shapes whose sequence leaves it untouched.
    pub(crate) fn operand_reads_and_writes(self) -> (Vec<u16>, Vec<u16>) {
        match self.row() {
            TrappingRow::Binary | TrappingRow::FixedPair | TrappingRow::Shift => {
                (vec![0, 1], vec![2, 3])
            }
            TrappingRow::Convert => (vec![0], vec![1, 2]),
        }
    }

    /// The encoded surface shared by the catalog and the footprint: the row's
    /// operand custody, an RFLAGS clobber, and the fall-through-or-trap
    /// control of the inline UD2 with no memory or stack effect.
    pub(crate) fn encoded_effects(self) -> MachineEncodedEffects {
        let (reads, writes) = self.operand_reads_and_writes();
        let mut encoded = MachineEncodedEffects::fallthrough_v1(reads, writes);
        encoded.implicit_unit_clobbers = crate::x86_64_physical_register_model()
            .view_named("rflags")
            .expect("canonical x86-64 model declares rflags")
            .units
            .clone();
        encoded.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        encoded.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
        encoded
    }
}

/// The Trapping form of a selected kind, or `None` for every other kind.
pub(crate) const fn trapping_form(kind: SelectedInstructionKind) -> Option<TrappingForm> {
    match kind {
        SelectedInstructionKind::TrappingInteger { form } => Some(form),
        _ => None,
    }
}
