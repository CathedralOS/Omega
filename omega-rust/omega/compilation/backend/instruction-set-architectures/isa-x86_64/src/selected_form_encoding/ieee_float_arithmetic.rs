//! Nearest-even scalar IEEE operations on general-register payloads.
//!
//! Scalar graphs keep IEEE values as raw payloads in general registers, so
//! one selected form moves or converts its operands into the fixed scratch
//! register `xmm4` (and `xmm5`), computes into `xmm4`, and moves the result
//! payload back with `MOVD`/`MOVQ` (a 32-bit move zero-extends the general
//! register). Every instruction is SSE2, the x86-64 baseline:
//!
//! - add, subtract, multiply, divide, minimum, maximum: both payloads in,
//!   one scalar `ADDS?`/`SUBS?`/`MULS?`/`DIVS?`/`MINS?`/`MAXS?`, one move
//!   out. `MINS? xmm4, xmm5` is `xmm4 < xmm5 ? xmm4 : xmm5` and returns its
//!   second operand on unordered or equal operands, which is exactly
//!   `FloatSemantics::minimum` (maximum likewise with `>`), NaN and signed
//!   zero included;
//! - negate: payload in, `PCMPEQD xmm5, xmm5` then `PSLLQ`/`PSLLD` to form
//!   the sign mask, `XORPD`/`XORPS`, move out. No memory constant is needed;
//! - square root and format conversion: payload in, one
//!   `SQRTS?`/`CVTSS2SD`/`CVTSD2SS`, move out;
//! - integer to float: one `CVTSI2S?` reading the general register (32-bit
//!   for `i32`, 64-bit for `i64`), move out. A `u32` operand is first copied
//!   with a 32-bit `MOV` into the result register, which zero-extends it, and
//!   converted from that 64-bit register, so every `u32` value is exact;
//! - float to integer: a flag-free clamped `CVTTSD2SI` (see `truncation`),
//!   saturating to the carrier and mapping NaN to zero. That is exactly the
//!   Saturating contract and the identity on every operand an Exact
//!   requirement's proof admits, so both domains share the instructions.
//!
//! Rounding operations round under MXCSR, which Omega's native boundaries
//! keep at the canonical value (round to nearest, ties to even; every
//! exception masked; no flush to zero), so each computes the exact IEEE-754
//! operation of its format and never traps. No instruction here writes
//! RFLAGS.
//!
//! `xmm4`/`xmm5` are volatile under both System V and Microsoft x64, so
//! clobbering them needs no callee-save obligation. The operations also
//! accumulate MXCSR's sticky exception flags; those stay outside the modeled
//! state because sticky status is not part of Omega's floating-control
//! invariant (`wiki/spec/build/foreign_bindings.md`, "Floating control
//! state"). Only the rounding-control read is declared.

use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, RegisterViewId,
    ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
};
use crate::selected_form_encoding::request_validation::resolve_registers;
use register_model::{RegisterConstraintKey, RegisterUnitId};
use selected_instructions::{
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatIntegerCarrier,
    MachineAlternative, MachineAlternativeApplicability, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineLatencyKnowledge, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

type Error = X86_64SelectedFormEncodingError;

/// The fixed scratch registers holding the first and second operands.
const LEFT_SCRATCH: u8 = 4;
const RIGHT_SCRATCH: u8 = 5;

fn view_units(name: &str) -> Vec<RegisterUnitId> {
    crate::x86_64_physical_register_model_ref()
        .view_named(name)
        .unwrap_or_else(|| panic!("canonical x86-64 model declares {name}"))
        .units
        .clone()
}

fn sorted(units: impl IntoIterator<Item = RegisterUnitId>) -> Vec<RegisterUnitId> {
    let mut units = units.into_iter().collect::<Vec<_>>();
    units.sort_unstable();
    units.dedup();
    units
}

/// MXCSR selects the rounding mode the operations honor.
pub(crate) fn implicit_uses() -> Vec<RegisterUnitId> {
    sorted(view_units("mxcsr"))
}

pub(crate) fn clobbers() -> Vec<RegisterUnitId> {
    sorted(view_units("xmm4").into_iter().chain(view_units("xmm5")))
}

/// The form reads its operands (operands `0..n`) and defines operand `n`.
pub(crate) fn effects(form: IeeeFloatArithmeticForm) -> MachineEncodedEffects {
    let inputs = form.operand_count() as u16;
    MachineEncodedEffects {
        implicit_unit_uses: implicit_uses(),
        implicit_unit_clobbers: clobbers(),
        ..MachineEncodedEffects::fallthrough_v1((0..inputs).collect(), vec![inputs])
    }
}

pub(crate) fn footprint(
    form: IeeeFloatArithmeticForm,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    let inputs = form.operand_count();
    X86_64SelectedFormFootprint {
        register_reads: operands[..inputs].to_vec(),
        register_writes: operands[inputs..inputs + 1].to_vec(),
        writes_rflags: false,
        encoded: effects(form),
    }
}

/// Encoded bytes: the shortest form uses only `rax`..`rdi`, the longest only
/// `r8`..`r15`, which adds one REX byte per general-register field.
fn size(form: IeeeFloatArithmeticForm) -> MachineSizeKnowledge {
    MachineSizeKnowledge::EncoderResolved {
        minimum_bytes: canonical(form, &[0, 0, 0]).len() as u16,
        maximum_bytes: Some(canonical(form, &[8, 8, 8]).len() as u16),
    }
}

pub(crate) fn declaration(
    form: IeeeFloatArithmeticForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::NearestIeeeFloatArithmetic(form),
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier: MachineBarrier::None,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: size(form),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(form),
        }],
    }
}

/// One instruction of a form, by its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Instruction {
    /// `66 [REX] 0F 6E|7E /r`: `MOVD`/`MOVQ` between `xmm` (ModRM.reg) and a
    /// general register (ModRM.rm); `wide` is REX.W.
    Transfer {
        to_float: bool,
        wide: bool,
        xmm: u8,
        general: u8,
    },
    /// `F2|F3 0F opcode /r` between two low scratch registers; `double` is
    /// the `F2` prefix.
    Scalar {
        double: bool,
        opcode: u8,
        destination: u8,
        source: u8,
    },
    /// `66 0F 76 /r`: `PCMPEQD xmm, xmm`, all ones.
    AllOnes { xmm: u8 },
    /// `66 0F 73|72 /6 ib`: `PSLLQ`/`PSLLD xmm, count`.
    ShiftLeft { quad: bool, xmm: u8, count: u8 },
    /// `[66] 0F 57 /r`: `XORPD`/`XORPS`.
    Xor {
        double: bool,
        destination: u8,
        source: u8,
    },
    /// `F2|F3 [REX] 0F 2A /r`: `CVTSI2S? xmm, r32/r64`.
    IntegerToFloat {
        double: bool,
        wide: bool,
        xmm: u8,
        general: u8,
    },
    /// `[REX] 89 /r`: `MOV r/m32, r32`, zero-extending the destination.
    Move32 { destination: u8, source: u8 },
    /// `66 0F 28 /r`: `MOVAPD xmm, xmm`.
    MoveAligned { destination: u8, source: u8 },
    /// `F2 0F C2 /r ib`: `CMPSD xmm, xmm, predicate`, an all-ones or zero
    /// mask in the destination's low lane.
    CompareScalar {
        predicate: u8,
        destination: u8,
        source: u8,
    },
    /// `66 0F 54 /r`: `ANDPD xmm, xmm`.
    And { destination: u8, source: u8 },
    /// `REX.W B8+r io`: `MOVABS r64, imm64`.
    MoveImmediate64 { general: u8, value: u64 },
    /// `F2 [REX] 0F 2C /r`: `CVTTSD2SI r32/r64, xmm`, toward zero; an
    /// unordered or out-of-range operand yields the integer indefinite value
    /// (the destination's minimum).
    FloatToInteger { wide: bool, general: u8, xmm: u8 },
    /// `REX.W 63 /r`: `MOVSXD r64, r32`.
    SignExtend32 { destination: u8, source: u8 },
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: Instruction) {
    let modrm = |reg: u8, rm: u8| 0xc0 | ((reg & 7) << 3) | (rm & 7);
    let optional_rex = |bytes: &mut Vec<u8>, rex: u8| {
        if rex != 0x40 {
            bytes.push(rex);
        }
    };
    let scalar_prefix = |double: bool| if double { 0xf2 } else { 0xf3 };
    match instruction {
        Instruction::Transfer {
            to_float,
            wide,
            xmm,
            general,
        } => {
            bytes.push(0x66);
            optional_rex(bytes, 0x40 | (u8::from(wide) << 3) | (general >> 3));
            bytes.extend([
                0x0f,
                if to_float { 0x6e } else { 0x7e },
                modrm(xmm, general),
            ]);
        }
        Instruction::Scalar {
            double,
            opcode,
            destination,
            source,
        } => bytes.extend([
            scalar_prefix(double),
            0x0f,
            opcode,
            modrm(destination, source),
        ]),
        Instruction::AllOnes { xmm } => bytes.extend([0x66, 0x0f, 0x76, modrm(xmm, xmm)]),
        Instruction::ShiftLeft { quad, xmm, count } => bytes.extend([
            0x66,
            0x0f,
            if quad { 0x73 } else { 0x72 },
            modrm(6, xmm),
            count,
        ]),
        Instruction::Xor {
            double,
            destination,
            source,
        } => {
            if double {
                bytes.push(0x66);
            }
            bytes.extend([0x0f, 0x57, modrm(destination, source)]);
        }
        Instruction::IntegerToFloat {
            double,
            wide,
            xmm,
            general,
        } => {
            bytes.push(scalar_prefix(double));
            optional_rex(bytes, 0x40 | (u8::from(wide) << 3) | (general >> 3));
            bytes.extend([0x0f, 0x2a, modrm(xmm, general)]);
        }
        Instruction::Move32 {
            destination,
            source,
        } => {
            optional_rex(bytes, 0x40 | ((source >> 3) << 2) | (destination >> 3));
            bytes.extend([0x89, modrm(source, destination)]);
        }
        Instruction::MoveAligned {
            destination,
            source,
        } => bytes.extend([0x66, 0x0f, 0x28, modrm(destination, source)]),
        Instruction::CompareScalar {
            predicate,
            destination,
            source,
        } => bytes.extend([0xf2, 0x0f, 0xc2, modrm(destination, source), predicate]),
        Instruction::And {
            destination,
            source,
        } => bytes.extend([0x66, 0x0f, 0x54, modrm(destination, source)]),
        Instruction::MoveImmediate64 { general, value } => {
            bytes.extend([0x48 | (general >> 3), 0xb8 | (general & 7)]);
            bytes.extend(value.to_le_bytes());
        }
        Instruction::FloatToInteger { wide, general, xmm } => {
            bytes.push(0xf2);
            optional_rex(bytes, 0x40 | (u8::from(wide) << 3) | ((general >> 3) << 2));
            bytes.extend([0x0f, 0x2c, modrm(general, xmm)]);
        }
        Instruction::SignExtend32 {
            destination,
            source,
        } => bytes.extend([
            0x48 | ((destination >> 3) << 2) | (source >> 3),
            0x63,
            modrm(destination, source),
        ]),
    }
}

/// Decode one instruction by its opcode bytes and fields, independently of
/// `encode_instruction`. A REX prefix that encodes nothing, REX.X, or REX.R
/// on an `xmm` field is not canonical here and rejects.
fn decode_instruction(bytes: &[u8]) -> Option<(Instruction, &[u8])> {
    let register_pair = |modrm: u8| (modrm & 0xc0 == 0xc0).then_some(((modrm >> 3) & 7, modrm & 7));
    fn optional_rex(bytes: &[u8]) -> (u8, &[u8]) {
        match bytes {
            [rex @ 0x41..=0x4f, rest @ ..] => (*rex, rest),
            _ => (0x40, bytes),
        }
    }
    let float_prefix = |prefix: u8| match prefix {
        0xf2 => Some(true),
        0xf3 => Some(false),
        _ => None,
    };
    match bytes {
        [0x66, 0x0f, 0x76, modrm, rest @ ..] => {
            let (reg, rm) = register_pair(*modrm)?;
            (reg == rm).then_some((Instruction::AllOnes { xmm: reg }, rest))
        }
        [0x66, 0x0f, opcode @ (0x72 | 0x73), modrm, count, rest @ ..] => {
            let (reg, rm) = register_pair(*modrm)?;
            (reg == 6).then_some((
                Instruction::ShiftLeft {
                    quad: *opcode == 0x73,
                    xmm: rm,
                    count: *count,
                },
                rest,
            ))
        }
        [0x66, 0x0f, 0x57, modrm, rest @ ..] | [0x0f, 0x57, modrm, rest @ ..] => {
            let (destination, source) = register_pair(*modrm)?;
            Some((
                Instruction::Xor {
                    double: bytes[0] == 0x66,
                    destination,
                    source,
                },
                rest,
            ))
        }
        [0x66, 0x0f, opcode @ (0x28 | 0x54), modrm, rest @ ..] => {
            let (destination, source) = register_pair(*modrm)?;
            Some((
                if *opcode == 0x28 {
                    Instruction::MoveAligned {
                        destination,
                        source,
                    }
                } else {
                    Instruction::And {
                        destination,
                        source,
                    }
                },
                rest,
            ))
        }
        [0xf2, 0x0f, 0xc2, modrm, predicate, rest @ ..] => {
            let (destination, source) = register_pair(*modrm)?;
            Some((
                Instruction::CompareScalar {
                    predicate: *predicate,
                    destination,
                    source,
                },
                rest,
            ))
        }
        [0x66, tail @ ..] => {
            let (rex, tail) = optional_rex(tail);
            let [0x0f, opcode @ (0x6e | 0x7e), modrm, rest @ ..] = tail else {
                return None;
            };
            let (xmm, general) = register_pair(*modrm)?;
            (rex & 6 == 0).then_some((
                Instruction::Transfer {
                    to_float: *opcode == 0x6e,
                    wide: rex & 8 != 0,
                    xmm,
                    general: general | ((rex & 1) << 3),
                },
                rest,
            ))
        }
        [prefix @ (0xf2 | 0xf3), tail @ ..] => {
            let double = float_prefix(*prefix)?;
            let (rex, tail) = optional_rex(tail);
            match tail {
                [0x0f, 0x2a, modrm, rest @ ..] if rex & 6 == 0 => {
                    let (xmm, general) = register_pair(*modrm)?;
                    Some((
                        Instruction::IntegerToFloat {
                            double,
                            wide: rex & 8 != 0,
                            xmm,
                            general: general | ((rex & 1) << 3),
                        },
                        rest,
                    ))
                }
                // REX.R names the general destination; REX.B would name
                // xmm8 or above.
                [0x0f, 0x2c, modrm, rest @ ..] if double && rex & 3 == 0 => {
                    let (general, xmm) = register_pair(*modrm)?;
                    Some((
                        Instruction::FloatToInteger {
                            wide: rex & 8 != 0,
                            general: general | (((rex >> 2) & 1) << 3),
                            xmm,
                        },
                        rest,
                    ))
                }
                [0x0f, opcode, modrm, rest @ ..] if rex == 0x40 => {
                    let (destination, source) = register_pair(*modrm)?;
                    Some((
                        Instruction::Scalar {
                            double,
                            opcode: *opcode,
                            destination,
                            source,
                        },
                        rest,
                    ))
                }
                _ => None,
            }
        }
        _ => {
            let (rex, tail) = optional_rex(bytes);
            let wide = rex & 0xa == 8;
            match tail {
                [0x89, modrm, rest @ ..] if rex & 0xa == 0 => {
                    let (source, destination) = register_pair(*modrm)?;
                    Some((
                        Instruction::Move32 {
                            destination: destination | ((rex & 1) << 3),
                            source: source | (((rex >> 2) & 1) << 3),
                        },
                        rest,
                    ))
                }
                [opcode @ 0xb8..=0xbf, rest @ ..] if wide && rex & 4 == 0 && rest.len() >= 8 => {
                    let (value, rest) = rest.split_at(8);
                    Some((
                        Instruction::MoveImmediate64 {
                            general: (opcode & 7) | ((rex & 1) << 3),
                            value: u64::from_le_bytes(value.try_into().ok()?),
                        },
                        rest,
                    ))
                }
                [0x63, modrm, rest @ ..] if wide => {
                    let (destination, source) = register_pair(*modrm)?;
                    Some((
                        Instruction::SignExtend32 {
                            destination: destination | (((rex >> 2) & 1) << 3),
                            source: source | ((rex & 1) << 3),
                        },
                        rest,
                    ))
                }
                _ => None,
            }
        }
    }
}

/// The scalar opcode of each two-operand operation.
const fn binary_opcode(operation: IeeeFloatArithmeticOperation) -> Option<u8> {
    match operation {
        IeeeFloatArithmeticOperation::Add => Some(0x58),
        IeeeFloatArithmeticOperation::Multiply => Some(0x59),
        IeeeFloatArithmeticOperation::Subtract => Some(0x5c),
        IeeeFloatArithmeticOperation::Minimum => Some(0x5d),
        IeeeFloatArithmeticOperation::Divide => Some(0x5e),
        IeeeFloatArithmeticOperation::Maximum => Some(0x5f),
        _ => None,
    }
}

/// The canonical instructions of one form: `registers` holds the operand
/// general registers in authored order followed by the result register.
fn program(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<Instruction> {
    let double = !form.is_binary32();
    let result = registers[form.operand_count()];
    let move_in = |general: u8, xmm: u8, wide: bool| Instruction::Transfer {
        to_float: true,
        wide,
        xmm,
        general,
    };
    let move_out = Instruction::Transfer {
        to_float: false,
        wide: double,
        xmm: LEFT_SCRATCH,
        general: result,
    };
    let in_place = |double: bool, opcode: u8| Instruction::Scalar {
        double,
        opcode,
        destination: LEFT_SCRATCH,
        source: LEFT_SCRATCH,
    };
    match form.operation {
        IeeeFloatArithmeticOperation::Negate => vec![
            move_in(registers[0], LEFT_SCRATCH, double),
            Instruction::AllOnes { xmm: RIGHT_SCRATCH },
            Instruction::ShiftLeft {
                quad: double,
                xmm: RIGHT_SCRATCH,
                count: if double { 63 } else { 31 },
            },
            Instruction::Xor {
                double,
                destination: LEFT_SCRATCH,
                source: RIGHT_SCRATCH,
            },
            move_out,
        ],
        IeeeFloatArithmeticOperation::SquareRoot => vec![
            move_in(registers[0], LEFT_SCRATCH, double),
            in_place(double, 0x51),
            move_out,
        ],
        // `CVTSS2SD` (`F3 0F 5A`) and `CVTSD2SS` (`F2 0F 5A`) are prefixed
        // by their source format, the other one.
        IeeeFloatArithmeticOperation::ConvertFormat => vec![
            move_in(registers[0], LEFT_SCRATCH, !double),
            in_place(!double, 0x5a),
            move_out,
        ],
        IeeeFloatArithmeticOperation::TruncateToInteger { target, .. } => {
            truncation(double, target, registers[0], result)
        }
        IeeeFloatArithmeticOperation::FromInteger(source) if !source.is_signed() => vec![
            Instruction::Move32 {
                destination: result,
                source: registers[0],
            },
            Instruction::IntegerToFloat {
                double,
                wide: true,
                xmm: LEFT_SCRATCH,
                general: result,
            },
            move_out,
        ],
        IeeeFloatArithmeticOperation::FromInteger(source) => vec![
            Instruction::IntegerToFloat {
                double,
                wide: source.bits() == 64,
                xmm: LEFT_SCRATCH,
                general: registers[0],
            },
            move_out,
        ],
        binary => vec![
            move_in(registers[0], LEFT_SCRATCH, double),
            move_in(registers[1], RIGHT_SCRATCH, double),
            Instruction::Scalar {
                double,
                opcode: binary_opcode(binary).expect("every remaining operation is binary"),
                destination: LEFT_SCRATCH,
                source: RIGHT_SCRATCH,
            },
            move_out,
        ],
    }
}

/// `2^31`, `2^63` and `2^32 - 1` as binary64 payloads: the first operand
/// value each signed carrier cannot hold, and the unsigned carrier's maximum.
const TWO_TO_31: u64 = 0x41e0_0000_0000_0000;
const TWO_TO_63: u64 = 0x43e0_0000_0000_0000;
const U32_MAXIMUM: u64 = 0x41ef_ffff_ffe0_0000;
/// `CMPSD` predicates: less-or-equal and ordered.
const LESS_OR_EQUAL: u8 = 2;
const ORDERED: u8 = 7;

/// Saturating toward-zero conversion of the operand payload in `source` to
/// `target`, without touching RFLAGS or memory. The operand is widened to
/// binary64 (exactly), a NaN is replaced by `+0` through an ordered mask, and
/// the result register doubles as the scratch that materializes each bound.
///
/// A signed carrier truncates with `CVTTSD2SI`, whose integer indefinite
/// value (the carrier's minimum) is already the saturated result of every
/// operand below the range; an operand at or above `2^31` (`2^63`) also
/// yields it, so a `bound <= operand` mask XORed over the result turns that
/// minimum into the maximum. `i32` is then sign-extended into its scalar
/// graph carrier. `u32` clamps the operand to `[+0, 2^32 - 1]` with
/// `MAXSD`/`MINSD` and truncates with the 64-bit `CVTTSD2SI`, whose result
/// then fits and leaves the upper half zero.
fn truncation(
    double: bool,
    target: IeeeFloatIntegerCarrier,
    source: u8,
    result: u8,
) -> Vec<Instruction> {
    let (left, right) = (LEFT_SCRATCH, RIGHT_SCRATCH);
    let mut program = vec![Instruction::Transfer {
        to_float: true,
        wide: double,
        xmm: left,
        general: source,
    }];
    if !double {
        // `CVTSS2SD xmm4, xmm4`: binary32 widens exactly.
        program.push(Instruction::Scalar {
            double: false,
            opcode: 0x5a,
            destination: left,
            source: left,
        });
    }
    program.extend([
        Instruction::MoveAligned {
            destination: right,
            source: left,
        },
        Instruction::CompareScalar {
            predicate: ORDERED,
            destination: right,
            source: left,
        },
        Instruction::And {
            destination: left,
            source: right,
        },
    ]);
    let bound = |value: u64| {
        [
            Instruction::MoveImmediate64 {
                general: result,
                value,
            },
            Instruction::Transfer {
                to_float: true,
                wide: true,
                xmm: right,
                general: result,
            },
        ]
    };
    if !target.is_signed() {
        program.extend([
            Instruction::Xor {
                double: true,
                destination: right,
                source: right,
            },
            // `MAXSD xmm4, xmm5`: negative operands and `-0` become `+0`.
            Instruction::Scalar {
                double: true,
                opcode: 0x5f,
                destination: left,
                source: right,
            },
        ]);
        program.extend(bound(U32_MAXIMUM));
        program.extend([
            // `MINSD xmm4, xmm5`.
            Instruction::Scalar {
                double: true,
                opcode: 0x5d,
                destination: left,
                source: right,
            },
            Instruction::FloatToInteger {
                wide: true,
                general: result,
                xmm: left,
            },
        ]);
        return program;
    }
    let wide = target.bits() == 64;
    program.extend(bound(if wide { TWO_TO_63 } else { TWO_TO_31 }));
    program.extend([
        Instruction::CompareScalar {
            predicate: LESS_OR_EQUAL,
            destination: right,
            source: left,
        },
        Instruction::FloatToInteger {
            wide,
            general: result,
            xmm: left,
        },
        Instruction::Transfer {
            to_float: true,
            wide,
            xmm: left,
            general: result,
        },
        Instruction::Xor {
            double: true,
            destination: left,
            source: right,
        },
        Instruction::Transfer {
            to_float: false,
            wide,
            xmm: left,
            general: result,
        },
    ]);
    if !wide {
        program.push(Instruction::SignExtend32 {
            destination: result,
            source: result,
        });
    }
    program
}

fn canonical(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    for instruction in program(form, registers) {
        encode_instruction(&mut bytes, instruction);
    }
    bytes
}

fn registers(
    physical: &ValidatedPhysicalRegisterModel,
    form: IeeeFloatArithmeticForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<Vec<u8>, Error> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(Error::NonCanonicalPhysicalModel);
    }
    if alternative
        != (MachineAlternativeKey {
            family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
            variant: 0,
        })
    {
        return Err(Error::AlternativeMismatch);
    }
    if operands.len() != form.operand_count() + 1 {
        return Err(Error::OperandCountMismatch);
    }
    resolve_registers(physical, operands)
}

pub(crate) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    form: IeeeFloatArithmeticForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let registers = registers(physical, form, alternative, operands)?;
    let bytes = canonical(form, &registers);
    validate(physical, form, alternative, operands, &bytes)
}

/// Decode every instruction's fields independently of `canonical` and
/// require exactly the form's program, then require the canonical prefix
/// choice as well.
pub(crate) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    form: IeeeFloatArithmeticForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let registers = registers(physical, form, alternative, operands)?;
    let mut decoded = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let (instruction, tail) = decode_instruction(rest).ok_or(Error::MalformedEncoding)?;
        decoded.push(instruction);
        rest = tail;
    }
    if decoded != program(form, &registers) || bytes != canonical(form, &registers) {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(form, operands),
    })
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
        IeeeFloatIntegerCarrier, MachineAlternativeFamily, MachineAlternativeKey,
    };

    fn key(form: IeeeFloatArithmeticForm) -> MachineAlternativeKey {
        MachineAlternativeKey {
            family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
            variant: 0,
        }
    }

    fn bytes_of(
        operation: IeeeFloatArithmeticOperation,
        format: IeeeFloatFormat,
        registers: &[&str],
    ) -> Vec<u8> {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let operands = registers
            .iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect::<Vec<_>>();
        let form = IeeeFloatArithmeticForm { operation, format };
        encode(&physical, form, key(form), &operands)
            .unwrap()
            .bytes()
            .to_vec()
    }

    #[test]
    fn encodings_match_the_architectural_bytes() {
        use IeeeFloatArithmeticOperation as Operation;
        use IeeeFloatFormat::{Binary32, Binary64};
        // movq xmm4, rcx; movq xmm5, r9; addsd xmm4, xmm5; movq rax, xmm4
        assert_eq!(
            bytes_of(Operation::Add, Binary64, &["rcx", "r9", "rax"]),
            [
                0x66, 0x48, 0x0f, 0x6e, 0xe1, 0x66, 0x49, 0x0f, 0x6e, 0xe9, 0xf2, 0x0f, 0x58, 0xe5,
                0x66, 0x48, 0x0f, 0x7e, 0xe0,
            ]
        );
        // movd xmm4, eax; movd xmm5, edx; divss xmm4, xmm5; movd r10d, xmm4
        assert_eq!(
            bytes_of(Operation::Divide, Binary32, &["rax", "rdx", "r10"]),
            [
                0x66, 0x0f, 0x6e, 0xe0, 0x66, 0x0f, 0x6e, 0xea, 0xf3, 0x0f, 0x5e, 0xe5, 0x66, 0x41,
                0x0f, 0x7e, 0xe2,
            ]
        );
        // minsd xmm4, xmm5 and maxss xmm4, xmm5
        assert_eq!(
            bytes_of(Operation::Minimum, Binary64, &["rcx", "rdx", "rax"])[10..14],
            [0xf2, 0x0f, 0x5d, 0xe5]
        );
        assert_eq!(
            bytes_of(Operation::Maximum, Binary32, &["rcx", "rdx", "rax"])[8..12],
            [0xf3, 0x0f, 0x5f, 0xe5]
        );
        // movq xmm4, rsi; pcmpeqd xmm5, xmm5; psllq xmm5, 63;
        // xorpd xmm4, xmm5; movq rdi, xmm4
        assert_eq!(
            bytes_of(Operation::Negate, Binary64, &["rsi", "rdi"]),
            [
                0x66, 0x48, 0x0f, 0x6e, 0xe6, 0x66, 0x0f, 0x76, 0xed, 0x66, 0x0f, 0x73, 0xf5, 0x3f,
                0x66, 0x0f, 0x57, 0xe5, 0x66, 0x48, 0x0f, 0x7e, 0xe7,
            ]
        );
        // movd xmm4, esi; pcmpeqd xmm5, xmm5; pslld xmm5, 31;
        // xorps xmm4, xmm5; movd edi, xmm4
        assert_eq!(
            bytes_of(Operation::Negate, Binary32, &["rsi", "rdi"]),
            [
                0x66, 0x0f, 0x6e, 0xe6, 0x66, 0x0f, 0x76, 0xed, 0x66, 0x0f, 0x72, 0xf5, 0x1f, 0x0f,
                0x57, 0xe5, 0x66, 0x0f, 0x7e, 0xe7,
            ]
        );
        // movq xmm4, r8; sqrtsd xmm4, xmm4; movq rax, xmm4
        assert_eq!(
            bytes_of(Operation::SquareRoot, Binary64, &["r8", "rax"]),
            [
                0x66, 0x49, 0x0f, 0x6e, 0xe0, 0xf2, 0x0f, 0x51, 0xe4, 0x66, 0x48, 0x0f, 0x7e, 0xe0,
            ]
        );
        // movd xmm4, ecx; cvtss2sd xmm4, xmm4; movq rax, xmm4
        assert_eq!(
            bytes_of(Operation::ConvertFormat, Binary64, &["rcx", "rax"]),
            [
                0x66, 0x0f, 0x6e, 0xe1, 0xf3, 0x0f, 0x5a, 0xe4, 0x66, 0x48, 0x0f, 0x7e, 0xe0,
            ]
        );
        // movq xmm4, rcx; cvtsd2ss xmm4, xmm4; movd eax, xmm4
        assert_eq!(
            bytes_of(Operation::ConvertFormat, Binary32, &["rcx", "rax"]),
            [
                0x66, 0x48, 0x0f, 0x6e, 0xe1, 0xf2, 0x0f, 0x5a, 0xe4, 0x66, 0x0f, 0x7e, 0xe0,
            ]
        );
        // cvtsi2sd xmm4, r9d; movq rax, xmm4
        assert_eq!(
            bytes_of(
                Operation::FromInteger(IeeeFloatIntegerCarrier::I32),
                Binary64,
                &["r9", "rax"]
            ),
            [0xf2, 0x41, 0x0f, 0x2a, 0xe1, 0x66, 0x48, 0x0f, 0x7e, 0xe0]
        );
        // cvtsi2ss xmm4, rcx; movd eax, xmm4
        assert_eq!(
            bytes_of(
                Operation::FromInteger(IeeeFloatIntegerCarrier::I64),
                Binary32,
                &["rcx", "rax"]
            ),
            [0xf3, 0x48, 0x0f, 0x2a, 0xe1, 0x66, 0x0f, 0x7e, 0xe0]
        );
        // mov r11d, ecx; cvtsi2sd xmm4, r11; movq r11, xmm4
        assert_eq!(
            bytes_of(
                Operation::FromInteger(IeeeFloatIntegerCarrier::U32),
                Binary64,
                &["rcx", "r11"]
            ),
            [
                0x41, 0x89, 0xcb, 0xf2, 0x49, 0x0f, 0x2a, 0xe3, 0x66, 0x49, 0x0f, 0x7e, 0xe3,
            ]
        );
    }

    /// Exact and Saturating truncation of one carrier and format share one
    /// realization; every other pair of forms differs in its bytes.
    fn shares_realization(left: IeeeFloatArithmeticForm, right: IeeeFloatArithmeticForm) -> bool {
        use IeeeFloatArithmeticOperation::TruncateToInteger;
        left == right
            || matches!(
                (left.operation, right.operation),
                (
                    TruncateToInteger { target: left_target, .. },
                    TruncateToInteger { target: right_target, .. },
                ) if left_target == right_target && left.format == right.format
            )
    }

    #[test]
    fn truncation_encodings_match_the_architectural_bytes() {
        use IeeeFloatArithmeticOperation::TruncateToInteger;
        use selected_instructions::IeeeFloatToIntegerDomain::Saturating;
        let truncate = |target| TruncateToInteger {
            target,
            domain: Saturating,
        };
        // movq xmm4, rcx; movapd xmm5, xmm4; cmpordsd xmm5, xmm4;
        // andpd xmm4, xmm5; movabs rax, 2^31; movq xmm5, rax;
        // cmplesd xmm5, xmm4; cvttsd2si eax, xmm4; movd xmm4, eax;
        // xorpd xmm4, xmm5; movd eax, xmm4; movsxd rax, eax
        assert_eq!(
            bytes_of(
                truncate(IeeeFloatIntegerCarrier::I32),
                IeeeFloatFormat::Binary64,
                &["rcx", "rax"]
            ),
            [
                0x66, 0x48, 0x0f, 0x6e, 0xe1, 0x66, 0x0f, 0x28, 0xec, 0xf2, 0x0f, 0xc2, 0xec, 0x07,
                0x66, 0x0f, 0x54, 0xe5, 0x48, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x41,
                0x66, 0x48, 0x0f, 0x6e, 0xe8, 0xf2, 0x0f, 0xc2, 0xec, 0x02, 0xf2, 0x0f, 0x2c, 0xc4,
                0x66, 0x0f, 0x6e, 0xe0, 0x66, 0x0f, 0x57, 0xe5, 0x66, 0x0f, 0x7e, 0xe0, 0x48, 0x63,
                0xc0,
            ]
        );
        // The binary32 operand widens first with cvtss2sd xmm4, xmm4.
        let widened = bytes_of(
            truncate(IeeeFloatIntegerCarrier::I64),
            IeeeFloatFormat::Binary32,
            &["rcx", "r9"],
        );
        assert_eq!(
            widened[..8],
            [0x66, 0x0f, 0x6e, 0xe1, 0xf3, 0x0f, 0x5a, 0xe4]
        );
        // movabs r9, 2^63 and the 64-bit cvttsd2si r9, xmm4.
        assert!(
            widened
                .windows(10)
                .any(|window| window == [0x49, 0xb9, 0, 0, 0, 0, 0, 0, 0xe0, 0x43])
        );
        assert!(
            widened
                .windows(5)
                .any(|window| window == [0xf2, 0x4c, 0x0f, 0x2c, 0xcc])
        );
        // u32 clamps with maxsd/minsd around movabs of 2^32 - 1 and ends in
        // the 64-bit cvttsd2si.
        let unsigned = bytes_of(
            truncate(IeeeFloatIntegerCarrier::U32),
            IeeeFloatFormat::Binary64,
            &["rcx", "r9"],
        );
        assert!(
            unsigned.windows(10).any(
                |window| window == [0x49, 0xb9, 0x00, 0x00, 0xe0, 0xff, 0xff, 0xff, 0xef, 0x41]
            )
        );
        assert!(unsigned.ends_with(&[0xf2, 0x0f, 0x5d, 0xe5, 0xf2, 0x4c, 0x0f, 0x2c, 0xcc]));
    }

    #[test]
    fn validation_binds_every_bit_operation_format_and_register() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let view = |name: &str| physical.model().view_named(name).unwrap().id;
        let all = [view("rsi"), view("r15"), view("r8")];
        for form in IeeeFloatArithmeticForm::ALL {
            let operands = &all[3 - form.operand_count() - 1..];
            let bytes = encode(&physical, form, key(form), operands)
                .unwrap()
                .bytes()
                .to_vec();
            for byte_index in 0..bytes.len() {
                for bit in 0..8 {
                    let mut changed = bytes.clone();
                    changed[byte_index] ^= 1 << bit;
                    assert!(
                        validate(&physical, form, key(form), operands, &changed).is_err(),
                        "{form:?} accepted byte {byte_index} bit {bit}"
                    );
                }
            }
            for other in IeeeFloatArithmeticForm::ALL {
                if other != form && other.operand_count() == form.operand_count() {
                    // Exact and Saturating truncation share their bytes; the
                    // alternative key still binds the exact form.
                    if !shares_realization(other, form) {
                        assert!(validate(&physical, other, key(other), operands, &bytes).is_err());
                    }
                    assert!(validate(&physical, form, key(other), operands, &bytes).is_err());
                }
            }
            let mut swapped = operands.to_vec();
            swapped.swap(0, operands.len() - 1);
            assert!(validate(&physical, form, key(form), &swapped, &bytes).is_err());
            assert!(validate(&physical, form, key(form), operands, &bytes[1..]).is_err());
            assert!(validate(&physical, form, key(form), &all[..1], &bytes).is_err());
        }
    }
}
