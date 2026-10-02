//! Explicit directed binary forms retain their format and mode in the same
//! IEEE catalog as nearest arithmetic. Each form saves the exact incoming
//! controls, installs its masked, gradual-underflow rounding mode, and restores
//! the saved controls before publishing its result. Independent instruction
//! decoding rejects a substituted mode, absent restore or unbalanced storage.
//!
//! Nearest-even scalar IEEE operations on general-register payloads.
//!
//! Scalar graphs keep IEEE values as raw payloads in general registers, so
//! one selected form moves or converts its operands into the fixed scratch
//! registers `v16`/`v17`, computes into `v16` (or `v18`), and `FMOV`s the
//! result payload back:
//!
//! - add, subtract, multiply, divide: two `FMOV`s in, one scalar
//!   `FADD`/`FSUB`/`FMUL`/`FDIV`, one `FMOV` out (16 bytes);
//! - multiply-then-add: two payloads in, `FMUL`, the addend in, `FADD`,
//!   result out (24 bytes). Separate instructions preserve both roundings;
//! - fused multiply-add: three payloads in, scalar `FMADD`, result out
//!   (20 bytes). The product is not rounded before the addition;
//! - minimum, maximum: two `FMOV`s in, `FCMGT` into `v18` (`right > left`
//!   for minimum, `left > right` for maximum), `BSL` selecting `v16` where
//!   the mask is set and `v17` elsewhere, one `FMOV` out (20 bytes). `FCMGT`
//!   is false on unordered and on equal operands, so the form returns the
//!   second operand exactly when `FloatSemantics::minimum`/`maximum` does,
//!   including NaN operands and `+0`/`-0`. `FMIN`/`FMINNM` would propagate a
//!   NaN or order `-0 < +0` and are deliberately not used; the mask route
//!   also leaves NZCV untouched;
//! - negate, square root, format conversion: one `FMOV` in, one scalar
//!   `FNEG`/`FSQRT`/`FCVT`, one `FMOV` out (12 bytes);
//! - NaN classification: payload in, `FCMEQ` comparing the operand with itself,
//!   mask out, carrier-width `ADD #1` (16 bytes). Equality produces all ones
//!   for non-NaNs and zero for NaNs; wrapping addition yields canonical false
//!   or true without writing NZCV or evaluating the operand twice;
//! - infinity classification: payload in, `FABS`, materialize positive infinity
//!   through the result register into `v17`, `FCMEQ`, mask out, `LSR` (28 bytes).
//!   Only either infinity equals positive infinity after sign removal; NaNs
//!   remain unordered. The top mask bit is canonical zero/one, without NZCV;
//! - integer to float: one `SCVTF`/`UCVTF` reading the general register
//!   directly (`w` for 32-bit and `x` for 64-bit sources), one `FMOV` out
//!   (8 bytes);
//! - float to integer: one `FMOV` in and one `FCVTZS`/`FCVTZU` writing the
//!   result register directly (8 bytes), plus `SXTW` for `i32`, whose scalar
//!   graph carrier is sign-extended (12 bytes). The conversion rounds toward
//!   zero, saturates to the destination width and maps NaN to zero: exactly
//!   the Saturating contract, and the identity on every operand an Exact
//!   requirement's proof admits. Trapping conversions first check ordered
//!   bounds with vector masks and branch over `BRK` only when each holds.
//!
//! Rounding operations round under FPCR, which Omega's native boundaries keep
//! at the canonical control state (round to nearest, ties to even; no trap
//! enables; no flush to zero), so each word computes the exact IEEE-754
//! operation of its format. Only explicitly Trapping conversions trap.
//!
//! `v16`..`v18` are caller-saved under AAPCS64 and are never argument homes,
//! so clobbering them needs no callee-save obligation. Both lanes of each are
//! clobbered because a scalar write into `d`/`s` clears the upper lane. The
//! operations also accumulate FPSR's sticky exception flags; like every other
//! floating operation they leave them outside the modeled state, because
//! sticky status is not part of Omega's floating-control invariant
//! (`wiki/spec/build/foreign_bindings.md`, "Floating control state").
//! Encodings follow Arm's A64 scalar floating-point data-processing (one, two
//! and three source), conversion between floating-point and integer, Advanced
//! SIMD scalar three-same `FCMGT`/`FCMEQ`, and three-same logical `BSL`.

use super::{Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint};
use register_model::{RegisterConstraintKey, RegisterUnitId, RegisterViewId};
use selected_instructions::{
    IeeeFloatArithmeticForm, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};
use selected_instructions::{
    IeeeFloatArithmeticOperation, IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain,
};

mod conversions;

/// The fixed scratch vector registers: the first and second operands, and
/// the minimum/maximum selection mask or fused multiply-add addend.
const LEFT_SCRATCH: u8 = 16;
const RIGHT_SCRATCH: u8 = 17;
const MASK_SCRATCH: u8 = 18;

fn sorted(units: impl IntoIterator<Item = RegisterUnitId>) -> Vec<RegisterUnitId> {
    let mut units = units.into_iter().collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}

fn view_units(name: &str) -> Vec<RegisterUnitId> {
    crate::aarch64_physical_register_model()
        .view_named(name)
        .unwrap_or_else(|| panic!("canonical AArch64 model declares {name}"))
        .units
        .clone()
}

/// FPCR selects the rounding mode the operations honor.
pub(crate) fn implicit_uses() -> Vec<RegisterUnitId> {
    sorted(view_units("fpcr"))
}

/// Both lanes of every scratch register.
pub(crate) fn clobbers() -> Vec<RegisterUnitId> {
    sorted(["q16", "q17", "q18"].into_iter().flat_map(view_units))
}

/// The form reads its operands (operands `0..n`) and defines operand `n`.
pub(crate) fn effects(form: IeeeFloatArithmeticForm) -> MachineEncodedEffects {
    let inputs = form.operand_count() as u16;
    let mut effects = MachineEncodedEffects {
        implicit_unit_uses: implicit_uses(),
        implicit_unit_clobbers: clobbers(),
        ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
    };
    if is_trapping(form) {
        effects.trap = MachineEncodedTrapBehavior::TrappingIntegerV1;
        effects.control = MachineEncodedControlEffect::FallThroughOrTrapV1;
    }
    effects
}

fn is_trapping(form: IeeeFloatArithmeticForm) -> bool {
    matches!(
        form.operation,
        IeeeFloatArithmeticOperation::TruncateToInteger {
            domain: IeeeFloatToIntegerDomain::Trapping,
            ..
        }
    )
}

pub(crate) fn declaration(
    form: IeeeFloatArithmeticForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::NearestIeeeFloatArithmetic(form),
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: if is_trapping(form) {
            MachineTrapBehavior::TrappingIntegerV1
        } else {
            MachineTrapBehavior::NeverV1
        },
        barrier: if is_trapping(form) {
            MachineBarrier::ExternalEffect
        } else {
            MachineBarrier::None
        },
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(4 * program(form, &[0, 0, 0, 0]).len() as u16),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(form),
        }],
    }
}

pub(crate) fn footprint(
    form: IeeeFloatArithmeticForm,
    operands: &[RegisterViewId],
) -> Aarch64SelectedFormFootprint {
    let inputs = form.operand_count();
    Aarch64SelectedFormFootprint {
        register_reads: operands[..inputs].to_vec(),
        register_writes: operands[inputs..inputs + 1].to_vec(),
        writes_nzcv: false,
        encoded: effects(form),
    }
}

/// One architectural word of a form, by its fields. `double` selects the
/// binary64 register width (`d`/`x`) over binary32 (`s`/`w`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Word {
    /// MRS/MSR of FPCR; sticky FPSR status is outside the control invariant.
    FloatingControl { read: bool, general: u8 },
    /// `CBNZ Xn, #8`: skip exactly the following `BRK` on a true mask.
    BranchNonzeroOverCrash { general: u8 },
    /// Omega's native Crash instruction, `BRK #0`.
    Crash,
    /// `FMOV` (general): general register to scalar vector register.
    ToFloat {
        double: bool,
        general: u8,
        vector: u8,
    },
    /// `FMOV` (general): scalar vector register to general register.
    ToGeneral {
        double: bool,
        vector: u8,
        general: u8,
    },
    /// Floating-point data-processing (two source), `opcode` bits 15..12.
    TwoSource {
        double: bool,
        opcode: u32,
        left: u8,
        right: u8,
        result: u8,
    },
    /// Scalar `FMADD`: one rounding of the exact product plus addend.
    FusedMultiplyAdd {
        double: bool,
        left: u8,
        right: u8,
        addend: u8,
        result: u8,
    },
    /// Floating-point data-processing (one source), `opcode` bits 20..15;
    /// `double` is the source type.
    OneSource {
        double: bool,
        opcode: u32,
        source: u8,
        result: u8,
    },
    /// `SCVTF`/`UCVTF` (scalar, integer).
    IntegerToFloat {
        wide: bool,
        double: bool,
        unsigned: bool,
        general: u8,
        vector: u8,
    },
    /// `FCVTZS`/`FCVTZU` (scalar, integer): toward-zero conversion that
    /// saturates to the destination width and maps NaN to zero.
    FloatToInteger {
        wide: bool,
        double: bool,
        unsigned: bool,
        vector: u8,
        general: u8,
    },
    /// `SXTW` (`SBFM Xd, Xn, #0, #31`): sign-extend a 32-bit result into the
    /// 64-bit carrier the scalar graph keeps for signed 32-bit values.
    SignExtend32 { source: u8, result: u8 },
    /// `SXTB`/`SXTH`/`UXTB`/`UXTH` (`SBFM`/`UBFM Wd, Wn, #0, #7|#15`):
    /// extend a narrow operand into the result register's low word.
    ExtendNarrow {
        signed: bool,
        halfword: bool,
        source: u8,
        result: u8,
    },
    /// `MOVZ`/`MOVK Xd, #immediate, LSL #(16 * shift)`.
    MoveWide {
        keep: bool,
        shift: u8,
        immediate: u16,
        general: u8,
    },
    /// `FCMEQ`/`FCMGT` (register, scalar): an all-ones or zero mask.
    Compare {
        equal: bool,
        double: bool,
        left: u8,
        right: u8,
        result: u8,
    },
    /// `BSL` (vector, 8B): `mask ? if_set : if_clear` per bit, into `mask`.
    BitwiseSelect { mask: u8, if_set: u8, if_clear: u8 },
    /// `ADD Wd|Xd, Wn|Xn, #immediate`, without changing NZCV.
    AddImmediate {
        wide: bool,
        immediate: u16,
        source: u8,
        result: u8,
    },
    /// `LSR Wd|Xd, Wn|Xn, #count` (`UBFM`), without changing NZCV.
    LogicalShiftRight {
        wide: bool,
        count: u8,
        source: u8,
        result: u8,
    },
}

fn encode_word(word: Word) -> u32 {
    let reg = |register: u8| u32::from(register);
    let width = |double: bool| u32::from(double);
    match word {
        Word::FloatingControl { read, general } => {
            (if read { 0xd53b_4400 } else { 0xd51b_4400 }) | reg(general)
        }
        Word::BranchNonzeroOverCrash { general } => 0xb500_0040 | reg(general),
        Word::Crash => 0xd420_0000,
        Word::ToFloat {
            double,
            general,
            vector,
        } => (if double { 0x9e67_0000 } else { 0x1e27_0000 }) | reg(general) << 5 | reg(vector),
        Word::ToGeneral {
            double,
            vector,
            general,
        } => (if double { 0x9e66_0000 } else { 0x1e26_0000 }) | reg(vector) << 5 | reg(general),
        Word::TwoSource {
            double,
            opcode,
            left,
            right,
            result,
        } => {
            0x1e20_0800
                | width(double) << 22
                | reg(right) << 16
                | opcode << 12
                | reg(left) << 5
                | reg(result)
        }
        Word::FusedMultiplyAdd {
            double,
            left,
            right,
            addend,
            result,
        } => {
            0x1f00_0000
                | width(double) << 22
                | reg(right) << 16
                | reg(addend) << 10
                | reg(left) << 5
                | reg(result)
        }
        Word::OneSource {
            double,
            opcode,
            source,
            result,
        } => 0x1e20_4000 | width(double) << 22 | opcode << 15 | reg(source) << 5 | reg(result),
        Word::IntegerToFloat {
            wide,
            double,
            unsigned,
            general,
            vector,
        } => {
            0x1e22_0000
                | width(wide) << 31
                | width(double) << 22
                | u32::from(unsigned) << 16
                | reg(general) << 5
                | reg(vector)
        }
        Word::FloatToInteger {
            wide,
            double,
            unsigned,
            vector,
            general,
        } => {
            0x1e38_0000
                | width(wide) << 31
                | width(double) << 22
                | u32::from(unsigned) << 16
                | reg(vector) << 5
                | reg(general)
        }
        Word::SignExtend32 { source, result } => 0x9340_7c00 | reg(source) << 5 | reg(result),
        Word::ExtendNarrow {
            signed,
            halfword,
            source,
            result,
        } => {
            (if signed { 0x1300_0000 } else { 0x5300_0000 })
                | (if halfword { 15 } else { 7 }) << 10
                | reg(source) << 5
                | reg(result)
        }
        Word::MoveWide {
            keep,
            shift,
            immediate,
            general,
        } => {
            (if keep { 0xf280_0000 } else { 0xd280_0000 })
                | u32::from(shift) << 21
                | u32::from(immediate) << 5
                | reg(general)
        }
        Word::Compare {
            equal,
            double,
            left,
            right,
            result,
        } => {
            (if equal { 0x5e20_e400 } else { 0x7ea0_e400 })
                | width(double) << 22
                | reg(right) << 16
                | reg(left) << 5
                | reg(result)
        }
        Word::AddImmediate {
            wide,
            immediate,
            source,
            result,
        } => {
            0x1100_0000
                | width(wide) << 31
                | u32::from(immediate) << 10
                | reg(source) << 5
                | reg(result)
        }
        Word::LogicalShiftRight {
            wide,
            count,
            source,
            result,
        } => {
            (if wide { 0xd340_fc00 } else { 0x5300_7c00 })
                | u32::from(count) << 16
                | reg(source) << 5
                | reg(result)
        }
        Word::BitwiseSelect {
            mask,
            if_set,
            if_clear,
        } => 0x2e60_1c00 | reg(if_clear) << 16 | reg(if_set) << 5 | reg(mask),
    }
}

/// Decode one word by its fixed opcode bits alone, independently of
/// `encode_word`; any word outside these classes is rejected.
fn decode_word(word: u32) -> Option<Word> {
    if word & !31 == 0xd53b_4400 || word & !31 == 0xd51b_4400 {
        return Some(Word::FloatingControl {
            read: word & !31 == 0xd53b_4400,
            general: (word & 31) as u8,
        });
    }
    let field = |shift: u32| ((word >> shift) & 31) as u8;
    let bit = |shift: u32| (word >> shift) & 1 == 1;
    let float_type = |shift: u32| match (word >> shift) & 3 {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    Some(match word & 0xffff_fc00 {
        _ if word == 0xd420_0000 => Word::Crash,
        _ if word & 0xffff_ffe0 == 0xb500_0040 => {
            Word::BranchNonzeroOverCrash { general: field(0) }
        }
        0x1e27_0000 | 0x9e67_0000 if bit(31) == bit(22) => Word::ToFloat {
            double: bit(31),
            general: field(5),
            vector: field(0),
        },
        0x1e26_0000 | 0x9e66_0000 if bit(31) == bit(22) => Word::ToGeneral {
            double: bit(31),
            vector: field(5),
            general: field(0),
        },
        _ if word & 0xff20_0c00 == 0x1e20_0800 => Word::TwoSource {
            double: float_type(22)?,
            opcode: (word >> 12) & 15,
            left: field(5),
            right: field(16),
            result: field(0),
        },
        _ if word & 0xff20_8000 == 0x1f00_0000 => Word::FusedMultiplyAdd {
            double: float_type(22)?,
            left: field(5),
            right: field(16),
            addend: field(10),
            result: field(0),
        },
        _ if word & 0xff20_7c00 == 0x1e20_4000 => Word::OneSource {
            double: float_type(22)?,
            opcode: (word >> 15) & 63,
            source: field(5),
            result: field(0),
        },
        _ if word & 0x7f3e_fc00 == 0x1e22_0000 => Word::IntegerToFloat {
            wide: bit(31),
            double: float_type(22)?,
            unsigned: bit(16),
            general: field(5),
            vector: field(0),
        },
        _ if word & 0x7f3e_fc00 == 0x1e38_0000 => Word::FloatToInteger {
            wide: bit(31),
            double: float_type(22)?,
            unsigned: bit(16),
            vector: field(5),
            general: field(0),
        },
        0x9340_7c00 => Word::SignExtend32 {
            source: field(5),
            result: field(0),
        },
        base @ (0x1300_1c00 | 0x1300_3c00 | 0x5300_1c00 | 0x5300_3c00) => Word::ExtendNarrow {
            signed: base & 0x4000_0000 == 0,
            halfword: base & 0x2000 != 0,
            source: field(5),
            result: field(0),
        },
        _ if word & 0xdf80_0000 == 0xd280_0000 => Word::MoveWide {
            keep: bit(29),
            shift: ((word >> 21) & 3) as u8,
            immediate: ((word >> 5) & 0xffff) as u16,
            general: field(0),
        },
        _ if matches!(word & 0xffa0_fc00, 0x7ea0_e400 | 0x5e20_e400) => Word::Compare {
            equal: word & 0xffa0_fc00 == 0x5e20_e400,
            double: bit(22),
            left: field(5),
            right: field(16),
            result: field(0),
        },
        _ if word & 0x7fc0_0000 == 0x1100_0000 => Word::AddImmediate {
            wide: bit(31),
            immediate: ((word >> 10) & 0xfff) as u16,
            source: field(5),
            result: field(0),
        },
        _ if matches!(word & 0xffc0_fc00, 0x5300_7c00 | 0xd340_fc00) => {
            let count = ((word >> 16) & 63) as u8;
            if !bit(31) && count >= 32 {
                return None;
            }
            Word::LogicalShiftRight {
                wide: bit(31),
                count,
                source: field(5),
                result: field(0),
            }
        }
        _ if word & 0xffe0_fc00 == 0x2e60_1c00 => Word::BitwiseSelect {
            mask: field(0),
            if_set: field(5),
            if_clear: field(16),
        },
        _ => return None,
    })
}

/// The two-source opcode field (bits 15..12) of each rounding operation.
const fn two_source_opcode(operation: IeeeFloatArithmeticOperation) -> Option<u32> {
    match operation {
        IeeeFloatArithmeticOperation::Multiply => Some(0b0000),
        IeeeFloatArithmeticOperation::Divide => Some(0b0001),
        IeeeFloatArithmeticOperation::Add => Some(0b0010),
        IeeeFloatArithmeticOperation::Subtract => Some(0b0011),
        _ => None,
    }
}

/// The canonical words of one form: `registers` holds the operand general
/// registers in authored order followed by the result register.
fn program(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<Word> {
    let double = !form.is_binary32();
    let result = registers[form.operand_count()];
    let move_in = |general: u8, vector: u8, double: bool| Word::ToFloat {
        double,
        general,
        vector,
    };
    let move_out = |vector: u8| Word::ToGeneral {
        double,
        vector,
        general: result,
    };
    let (first, second, mask) = (LEFT_SCRATCH, RIGHT_SCRATCH, MASK_SCRATCH);
    match form.operation {
        IeeeFloatArithmeticOperation::DirectedBinary {
            operation,
            rounding,
        } => {
            // Capture both operands before borrowing the output as a GPR
            // temporary. q18 holds the exact saved FPCR until restoration;
            // no additional allocatable register or stack home is consumed.
            let mode = match rounding.ordinal() {
                0 => 3,
                1 => 1,
                2 => 2,
                _ => unreachable!(),
            };
            let opcode = match operation.ordinal() {
                0 => 2,
                1 => 3,
                2 => 0,
                3 => 1,
                _ => unreachable!(),
            };
            vec![
                move_in(registers[0], first, double),
                move_in(registers[1], second, double),
                Word::FloatingControl {
                    read: true,
                    general: result,
                },
                move_in(result, mask, true),
                Word::MoveWide {
                    keep: false,
                    shift: 1,
                    immediate: mode << 6,
                    general: result,
                },
                Word::FloatingControl {
                    read: false,
                    general: result,
                },
                Word::TwoSource {
                    double,
                    opcode,
                    left: first,
                    right: second,
                    result: first,
                },
                Word::ToGeneral {
                    double: true,
                    vector: mask,
                    general: result,
                },
                Word::FloatingControl {
                    read: false,
                    general: result,
                },
                move_out(first),
            ]
        }
        IeeeFloatArithmeticOperation::IsInfinite => vec![
            move_in(registers[0], first, double),
            Word::OneSource {
                double,
                opcode: 1,
                source: first,
                result: first,
            },
            // Input has already moved, so the output may safely hold the
            // infinity constant even when both operands name one register.
            Word::MoveWide {
                keep: false,
                shift: if double { 3 } else { 1 },
                immediate: if double { 0x7ff0 } else { 0x7f80 },
                general: result,
            },
            move_in(result, second, double),
            Word::Compare {
                equal: true,
                double,
                left: first,
                right: second,
                result: first,
            },
            move_out(first),
            Word::LogicalShiftRight {
                wide: double,
                count: if double { 63 } else { 31 },
                source: result,
                result,
            },
        ],
        IeeeFloatArithmeticOperation::IsNan => vec![
            move_in(registers[0], first, double),
            Word::Compare {
                equal: true,
                double,
                left: first,
                right: first,
                result: first,
            },
            move_out(first),
            // Equality with itself is false only for NaN. Increment its
            // all-ones/zero mask at the carrier width to obtain exactly 0/1,
            // with no flags changed and no second evaluation of the operand.
            Word::AddImmediate {
                wide: double,
                immediate: 1,
                source: result,
                result,
            },
        ],
        IeeeFloatArithmeticOperation::FusedMultiplyAdd => vec![
            move_in(registers[0], first, double),
            move_in(registers[1], second, double),
            move_in(registers[2], mask, double),
            Word::FusedMultiplyAdd {
                double,
                left: first,
                right: second,
                addend: mask,
                result: first,
            },
            move_out(first),
        ],
        IeeeFloatArithmeticOperation::MultiplyThenAdd => vec![
            move_in(registers[0], first, double),
            move_in(registers[1], second, double),
            // Two architectural roundings, never FMADD. The addend is read
            // before writing the general result, so result/operand aliasing
            // remains legal under the ordinary register constraint.
            Word::TwoSource {
                double,
                opcode: 0b0000,
                left: first,
                right: second,
                result: first,
            },
            move_in(registers[2], second, double),
            Word::TwoSource {
                double,
                opcode: 0b0010,
                left: first,
                right: second,
                result: first,
            },
            move_out(first),
        ],
        IeeeFloatArithmeticOperation::Minimum | IeeeFloatArithmeticOperation::Maximum => {
            // Minimum selects the left operand where `right > left`; maximum
            // where `left > right`. Both fall back to the right operand.
            let (left, right) = if form.operation == IeeeFloatArithmeticOperation::Minimum {
                (second, first)
            } else {
                (first, second)
            };
            vec![
                move_in(registers[0], first, double),
                move_in(registers[1], second, double),
                Word::Compare {
                    equal: false,
                    double,
                    left,
                    right,
                    result: mask,
                },
                Word::BitwiseSelect {
                    mask,
                    if_set: first,
                    if_clear: second,
                },
                move_out(mask),
            ]
        }
        IeeeFloatArithmeticOperation::Negate | IeeeFloatArithmeticOperation::SquareRoot => {
            let opcode = if form.operation == IeeeFloatArithmeticOperation::Negate {
                0b00_0010
            } else {
                0b00_0011
            };
            vec![
                move_in(registers[0], first, double),
                Word::OneSource {
                    double,
                    opcode,
                    source: first,
                    result: first,
                },
                move_out(first),
            ]
        }
        IeeeFloatArithmeticOperation::ConvertFormat => vec![
            // `FCVT` names its destination type in `opc` and its source type
            // in `ftype`: binary64 destination is `0b000101`, binary32 is
            // `0b000100`, and the source is the other format.
            move_in(registers[0], first, !double),
            Word::OneSource {
                double: !double,
                opcode: if double { 0b00_0101 } else { 0b00_0100 },
                source: first,
                result: first,
            },
            move_out(first),
        ],
        IeeeFloatArithmeticOperation::TruncateToInteger {
            target,
            domain: IeeeFloatToIntegerDomain::Trapping,
        } => conversions::trapping_truncation(double, target, registers[0], result),
        IeeeFloatArithmeticOperation::TruncateToInteger { target, .. } if target.bits() < 32 => {
            narrow_truncation(double, target, registers[0], result)
        }
        IeeeFloatArithmeticOperation::FromInteger(source) if source.bits() < 32 => vec![
            // Extend explicitly rather than rely on the register carrier's
            // upper bits, then convert the exact 32-bit value.
            Word::ExtendNarrow {
                signed: source.is_signed(),
                halfword: source.bits() == 16,
                source: registers[0],
                result,
            },
            Word::IntegerToFloat {
                wide: false,
                double,
                unsigned: !source.is_signed(),
                general: result,
                vector: first,
            },
            move_out(first),
        ],
        IeeeFloatArithmeticOperation::TruncateToInteger { target, .. } => {
            // Exact and Saturating realize alike: the conversion saturates
            // and maps NaN to zero, which is the identity on every operand an
            // Exact requirement's proof admits.
            let mut words = vec![
                move_in(registers[0], first, double),
                Word::FloatToInteger {
                    wide: target.bits() == 64,
                    double,
                    unsigned: !target.is_signed(),
                    vector: first,
                    general: result,
                },
            ];
            if target.is_signed() && target.bits() == 32 {
                words.push(Word::SignExtend32 {
                    source: result,
                    result,
                });
            }
            words
        }
        IeeeFloatArithmeticOperation::FromInteger(source) => vec![
            Word::IntegerToFloat {
                wide: source.bits() == 64,
                double,
                unsigned: !source.is_signed(),
                general: registers[0],
                vector: first,
            },
            move_out(first),
        ],
        arithmetic => {
            let opcode = two_source_opcode(arithmetic)
                .expect("every remaining operation is two-source arithmetic");
            vec![
                move_in(registers[0], first, double),
                move_in(registers[1], second, double),
                Word::TwoSource {
                    double,
                    opcode,
                    left: first,
                    right: second,
                    result: first,
                },
                move_out(first),
            ]
        }
    }
}

/// The binary64 payloads of a narrow carrier's minimum and maximum.
const fn narrow_bounds(target: IeeeFloatIntegerCarrier) -> (u64, u64) {
    match target {
        IeeeFloatIntegerCarrier::I8 => (0xc060_0000_0000_0000, 0x405f_c000_0000_0000),
        IeeeFloatIntegerCarrier::I16 => (0xc0e0_0000_0000_0000, 0x40df_ffc0_0000_0000),
        IeeeFloatIntegerCarrier::U8 => (0, 0x406f_e000_0000_0000),
        IeeeFloatIntegerCarrier::U16 => (0, 0x40ef_ffe0_0000_0000),
        _ => (0, 0),
    }
}

/// `MOVZ` of the top halfword and `MOVK` of each other nonzero halfword.
fn materialize(bits: u64, general: u8) -> Vec<Word> {
    let mut words = vec![Word::MoveWide {
        keep: false,
        shift: 3,
        immediate: (bits >> 48) as u16,
        general,
    }];
    for shift in (0..3).rev() {
        let immediate = (bits >> (16 * shift)) as u16;
        if immediate != 0 {
            words.push(Word::MoveWide {
                keep: true,
                shift,
                immediate,
                general,
            });
        }
    }
    words
}

/// Saturating toward-zero conversion to an 8- or 16-bit carrier. The operand
/// widens to binary64 exactly, clamps to the carrier's range with `FMAX` and
/// `FMIN` (which propagate a NaN), and converts with the 32-bit
/// `FCVTZS`/`FCVTZU`, which maps that NaN to zero; a signed result is then
/// sign-extended into its 64-bit carrier. Each bound is materialized in the
/// result register, which the form defines anyway, and moved into `v17`.
fn narrow_truncation(
    double: bool,
    target: IeeeFloatIntegerCarrier,
    source: u8,
    result: u8,
) -> Vec<Word> {
    let (first, second) = (LEFT_SCRATCH, RIGHT_SCRATCH);
    let mut words = vec![Word::ToFloat {
        double,
        general: source,
        vector: first,
    }];
    if !double {
        // `FCVT d16, s16`: binary32 widens exactly.
        words.push(Word::OneSource {
            double: false,
            opcode: 0b00_0101,
            source: first,
            result: first,
        });
    }
    let (minimum, maximum) = narrow_bounds(target);
    for (bound, opcode) in [(minimum, 0b0100), (maximum, 0b0101)] {
        words.extend(materialize(bound, result));
        words.push(Word::ToFloat {
            double: true,
            general: result,
            vector: second,
        });
        // `FMAX` then `FMIN d16, d16, d17`.
        words.push(Word::TwoSource {
            double: true,
            opcode,
            left: first,
            right: second,
            result: first,
        });
    }
    words.push(Word::FloatToInteger {
        wide: false,
        double: true,
        unsigned: !target.is_signed(),
        vector: first,
        general: result,
    });
    if target.is_signed() {
        words.push(Word::SignExtend32 {
            source: result,
            result,
        });
    }
    words
}

fn validate_registers(
    form: IeeeFloatArithmeticForm,
    registers: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if registers.len() != form.operand_count() + 1 {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    }
    // Register 31 is SP/ZR in these fields, never an allocatable payload home.
    if registers.iter().any(|register| *register > 30) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

pub(crate) fn encode(
    form: IeeeFloatArithmeticForm,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    validate_registers(form, registers)?;
    Ok(program(form, registers)
        .into_iter()
        .flat_map(|word| encode_word(word).to_le_bytes())
        .collect())
}

/// Decode every word's fields independently of `encode` and require exactly
/// the form's canonical program: the move directions and widths, the scratch
/// registers, each operation's type and opcode, and each general register.
pub(crate) fn validate(
    form: IeeeFloatArithmeticForm,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    validate_registers(form, registers)?;
    let (words, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let decoded = words
        .iter()
        .map(|word| decode_word(u32::from_le_bytes(*word)))
        .collect::<Option<Vec<_>>>()
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    if decoded != program(form, registers) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
        IeeeFloatIntegerCarrier, IeeeFloatToIntegerDomain,
    };

    fn form(
        operation: IeeeFloatArithmeticOperation,
        format: IeeeFloatFormat,
    ) -> IeeeFloatArithmeticForm {
        IeeeFloatArithmeticForm { operation, format }
    }

    fn words(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<u32> {
        encode(form, registers)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect()
    }

    #[test]
    fn directed_division_uses_output_after_capturing_inputs_and_restores_fpcr() {
        for (operation, expected_mode) in [(54, 3), (55, 1), (56, 2)] {
            for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                let form = form(IeeeFloatArithmeticOperation::ALL[operation], format);
                for registers in [[1, 2, 1], [1, 2, 2], [0, 0, 0], [28, 29, 30]] {
                    let program = super::program(form, &registers);
                    assert_eq!(
                        program[2],
                        super::Word::FloatingControl {
                            read: true,
                            general: registers[2]
                        }
                    );
                    assert_eq!(
                        program[4],
                        super::Word::MoveWide {
                            keep: false,
                            shift: 1,
                            immediate: expected_mode << 6,
                            general: registers[2]
                        }
                    );
                    assert_eq!(
                        program[8],
                        super::Word::FloatingControl {
                            read: false,
                            general: registers[2]
                        }
                    );
                    let bytes = super::encode(form, &registers).unwrap();
                    super::validate(form, &registers, &bytes).unwrap();
                    assert_eq!(
                        super::effects(form).stack,
                        selected_instructions::MachineEncodedStackEffect::UnchangedV1
                    );
                }
            }
        }
    }

    #[test]
    fn fused_multiply_add_preserves_three_sources_and_one_rounding_instruction() {
        use IeeeFloatArithmeticOperation::{FusedMultiplyAdd, MultiplyThenAdd};
        use IeeeFloatFormat::{Binary32, Binary64};
        // fmov s16,w1; fmov s17,w2; fmov s18,w3;
        // fmadd s16,s16,s17,s18; fmov w0,s16, and the double-width form.
        for (format, expected) in [
            (
                Binary32,
                [
                    0x1e27_0030,
                    0x1e27_0051,
                    0x1e27_0072,
                    0x1f11_4a10,
                    0x1e26_0200,
                ],
            ),
            (
                Binary64,
                [
                    0x9e67_0030,
                    0x9e67_0051,
                    0x9e67_0072,
                    0x1f51_4a10,
                    0x9e66_0200,
                ],
            ),
        ] {
            let fused = form(FusedMultiplyAdd, format);
            assert_eq!(words(fused, &[1, 2, 3, 0]), expected);
            for registers in [[1, 2, 3, 1], [1, 2, 3, 2], [1, 2, 3, 3], [0, 0, 0, 0]] {
                let bytes = encode(fused, &registers).unwrap();
                validate(fused, &registers, &bytes).unwrap();
                let unfused = encode(form(MultiplyThenAdd, format), &registers).unwrap();
                assert!(validate(fused, &registers, &unfused).is_err());
                assert!(validate(form(MultiplyThenAdd, format), &registers, &bytes).is_err());
            }
        }
    }

    #[test]
    fn encodings_match_the_architectural_words() {
        use IeeeFloatArithmeticOperation as Operation;
        use IeeeFloatFormat::{Binary32, Binary64};
        // Independently assembled, with the input and result aliased.
        assert_eq!(
            words(form(Operation::IsInfinite, Binary32), &[0, 0]),
            [
                0x1e27_0010,
                0x1e20_c210,
                0xd2af_f000,
                0x1e27_0011,
                0x5e31_e610,
                0x1e26_0200,
                0x531f_7c00
            ]
        );
        assert_eq!(
            words(form(Operation::IsInfinite, Binary64), &[0, 0]),
            [
                0x9e67_0010,
                0x1e60_c210,
                0xd2ef_fe00,
                0x9e67_0011,
                0x5e71_e610,
                0x9e66_0200,
                0xd37f_fc00
            ]
        );
        // Independently assembled: FMOV; FCMEQ self; FMOV; ADD #1.
        assert_eq!(
            words(form(Operation::IsNan, Binary32), &[0, 0]),
            [0x1e27_0010, 0x5e30_e610, 0x1e26_0200, 0x1100_0400]
        );
        assert_eq!(
            words(form(Operation::IsNan, Binary64), &[0, 0]),
            [0x9e67_0010, 0x5e70_e610, 0x9e66_0200, 0x9100_0400]
        );
        // fmov d16,x1; fmov d17,x2; fmul d16,d16,d17;
        // fmov d17,x3; fadd d16,d16,d17; fmov x0,d16.
        assert_eq!(
            words(form(Operation::MultiplyThenAdd, Binary64), &[1, 2, 3, 0]),
            [
                0x9e67_0030,
                0x9e67_0051,
                0x1e71_0a10,
                0x9e67_0071,
                0x1e71_2a10,
                0x9e66_0200
            ]
        );
        // fmov d16, x1; fmov d17, x2; fadd d16, d16, d17; fmov x0, d16
        assert_eq!(
            words(form(Operation::Add, Binary64), &[1, 2, 0]),
            [0x9e67_0030, 0x9e67_0051, 0x1e71_2a10, 0x9e66_0200]
        );
        // fmov s16, w3; fmov s17, w4; fdiv s16, s16, s17; fmov w5, s16
        assert_eq!(
            words(form(Operation::Divide, Binary32), &[3, 4, 5]),
            [0x1e27_0070, 0x1e27_0091, 0x1e31_1a10, 0x1e26_0205]
        );
        assert_eq!(
            words(form(Operation::Subtract, Binary64), &[0, 0, 0])[2],
            0x1e71_3a10
        );
        assert_eq!(
            words(form(Operation::Multiply, Binary32), &[0, 0, 0])[2],
            0x1e31_0a10
        );
        // fmov d16, x1; fmov d17, x2; fcmgt d18, d17, d16;
        // bsl v18.8b, v16.8b, v17.8b; fmov x0, d18
        assert_eq!(
            words(form(Operation::Minimum, Binary64), &[1, 2, 0]),
            [
                0x9e67_0030,
                0x9e67_0051,
                0x7ef0_e632,
                0x2e71_1e12,
                0x9e66_0240
            ]
        );
        // fcmgt s18, s16, s17 selects the left operand where it is greater.
        assert_eq!(
            words(form(Operation::Maximum, Binary32), &[1, 2, 0])[2],
            0x7eb1_e612
        );
        // fmov d16, x3; fneg d16, d16; fmov x4, d16
        assert_eq!(
            words(form(Operation::Negate, Binary64), &[3, 4]),
            [0x9e67_0070, 0x1e61_4210, 0x9e66_0204]
        );
        // fmov s16, w3; fsqrt s16, s16; fmov w4, s16
        assert_eq!(
            words(form(Operation::SquareRoot, Binary32), &[3, 4]),
            [0x1e27_0070, 0x1e21_c210, 0x1e26_0204]
        );
        // fmov s16, w3; fcvt d16, s16; fmov x4, d16
        assert_eq!(
            words(form(Operation::ConvertFormat, Binary64), &[3, 4]),
            [0x1e27_0070, 0x1e22_c210, 0x9e66_0204]
        );
        // fmov d16, x3; fcvt s16, d16; fmov w4, s16
        assert_eq!(
            words(form(Operation::ConvertFormat, Binary32), &[3, 4]),
            [0x9e67_0070, 0x1e62_4210, 0x1e26_0204]
        );
        // scvtf d16, w3; fmov x4, d16
        assert_eq!(
            words(
                form(
                    Operation::FromInteger(IeeeFloatIntegerCarrier::I32),
                    Binary64
                ),
                &[3, 4]
            ),
            [0x1e62_0070, 0x9e66_0204]
        );
        // scvtf s16, x3; fmov w4, s16
        assert_eq!(
            words(
                form(
                    Operation::FromInteger(IeeeFloatIntegerCarrier::I64),
                    Binary32
                ),
                &[3, 4]
            ),
            [0x9e22_0070, 0x1e26_0204]
        );
        // ucvtf d16, w3; fmov x4, d16
        assert_eq!(
            words(
                form(
                    Operation::FromInteger(IeeeFloatIntegerCarrier::U32),
                    Binary64
                ),
                &[3, 4]
            ),
            [0x1e63_0070, 0x9e66_0204]
        );
    }

    /// Exact and Saturating truncation of one carrier and format share one
    /// realization; every other pair of forms differs in its words.
    fn shares_realization(left: IeeeFloatArithmeticForm, right: IeeeFloatArithmeticForm) -> bool {
        use IeeeFloatArithmeticOperation::TruncateToInteger;
        left == right
            || matches!(
                (left.operation, right.operation),
                (
                    TruncateToInteger { target: left_target, domain: IeeeFloatToIntegerDomain::Exact | IeeeFloatToIntegerDomain::Saturating },
                    TruncateToInteger { target: right_target, domain: IeeeFloatToIntegerDomain::Exact | IeeeFloatToIntegerDomain::Saturating },
                ) if left_target == right_target && left.format == right.format
            )
    }

    #[test]
    fn truncation_encodings_match_the_architectural_words() {
        use IeeeFloatArithmeticOperation::TruncateToInteger;
        use selected_instructions::IeeeFloatToIntegerDomain::{Exact, Saturating};
        // fmov d16, x3; fcvtzs w4, d16; sxtw x4, w4
        assert_eq!(
            words(
                form(
                    TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::I32,
                        domain: Saturating,
                    },
                    IeeeFloatFormat::Binary64,
                ),
                &[3, 4]
            ),
            [0x9e67_0070, 0x1e78_0204, 0x9340_7c84]
        );
        // fmov s16, w3; fcvtzs x4, s16
        assert_eq!(
            words(
                form(
                    TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::I64,
                        domain: Exact,
                    },
                    IeeeFloatFormat::Binary32,
                ),
                &[3, 4]
            ),
            [0x1e27_0070, 0x9e38_0204]
        );
        // fmov d16, x3; fcvtzu w4, d16
        assert_eq!(
            words(
                form(
                    TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::U32,
                        domain: Saturating,
                    },
                    IeeeFloatFormat::Binary64,
                ),
                &[3, 4]
            ),
            [0x9e67_0070, 0x1e79_0204]
        );
    }

    #[test]
    fn narrow_carrier_encodings_match_the_architectural_words() {
        use IeeeFloatArithmeticOperation::{FromInteger, TruncateToInteger};
        use selected_instructions::IeeeFloatToIntegerDomain::Saturating;
        // sxtb w4, w3; scvtf d16, w4; fmov x4, d16
        assert_eq!(
            words(
                form(
                    FromInteger(IeeeFloatIntegerCarrier::I8),
                    IeeeFloatFormat::Binary64
                ),
                &[3, 4]
            ),
            [0x1300_1c64, 0x1e62_0090, 0x9e66_0204]
        );
        // uxth w4, w3; ucvtf s16, w4; fmov w4, s16
        assert_eq!(
            words(
                form(
                    FromInteger(IeeeFloatIntegerCarrier::U16),
                    IeeeFloatFormat::Binary32
                ),
                &[3, 4]
            ),
            [0x5300_3c64, 0x1e23_0090, 0x1e26_0204]
        );
        // fmov d16, x3; movz x4, #0xc060, lsl #48; fmov d17, x4;
        // fmax d16, d16, d17; movz x4, #0x405f, lsl #48;
        // movk x4, #0xc000, lsl #32; fmov d17, x4; fmin d16, d16, d17;
        // fcvtzs w4, d16; sxtw x4, w4
        assert_eq!(
            words(
                form(
                    TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::I8,
                        domain: Saturating,
                    },
                    IeeeFloatFormat::Binary64,
                ),
                &[3, 4]
            ),
            [
                0x9e67_0070,
                0xd2f8_0c04,
                0x9e67_0091,
                0x1e71_4a10,
                0xd2e8_0be4,
                0xf2d8_0004,
                0x9e67_0091,
                0x1e71_5a10,
                0x1e78_0204,
                0x9340_7c84
            ]
        );
    }

    #[test]
    fn validation_binds_every_bit_operation_format_and_register() {
        for form in IeeeFloatArithmeticForm::ALL {
            let registers = &[6, 7, 30, 12][4 - form.operand_count() - 1..];
            let bytes = encode(form, registers).unwrap();
            validate(form, registers, &bytes).unwrap();
            for byte_index in 0..bytes.len() {
                for bit in 0..8 {
                    let mut changed = bytes.clone();
                    changed[byte_index] ^= 1 << bit;
                    assert!(
                        validate(form, registers, &changed).is_err(),
                        "{form:?} accepted byte {byte_index} bit {bit}"
                    );
                }
            }
            for other in IeeeFloatArithmeticForm::ALL {
                if other.operand_count() == form.operand_count() && !shares_realization(other, form)
                {
                    assert!(
                        validate(other, registers, &bytes).is_err(),
                        "{other:?} as {form:?}"
                    );
                }
            }
            let mut result_changed = registers.to_vec();
            *result_changed.last_mut().unwrap() += 1;
            assert!(validate(form, &result_changed, &bytes).is_err());
            let mut operands_swapped = registers.to_vec();
            operands_swapped.swap(0, registers.len() - 1);
            assert!(validate(form, &operands_swapped, &bytes).is_err());
            let mut wide = vec![31];
            wide.extend_from_slice(&registers[1..]);
            assert!(encode(form, &wide).is_err());
            assert!(encode(form, &[1, 2, 3, 4, 5]).is_err());
            assert!(validate(form, registers, &bytes[..bytes.len() - 4]).is_err());
        }
    }
}
