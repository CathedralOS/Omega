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
//!   converted from that 64-bit register, so every `u32` value is exact.
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
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, MachineAlternative,
    MachineAlternativeApplicability, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior,
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
            let [0x89, modrm, rest @ ..] = tail else {
                return None;
            };
            let (source, destination) = register_pair(*modrm)?;
            (rex & 0xa == 0).then_some((
                Instruction::Move32 {
                    destination: destination | ((rex & 1) << 3),
                    source: source | (((rex >> 2) & 1) << 3),
                },
                rest,
            ))
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
        IeeeFloatIntegerSource, MachineAlternativeFamily, MachineAlternativeKey,
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
                Operation::FromInteger(IeeeFloatIntegerSource::I32),
                Binary64,
                &["r9", "rax"]
            ),
            [0xf2, 0x41, 0x0f, 0x2a, 0xe1, 0x66, 0x48, 0x0f, 0x7e, 0xe0]
        );
        // cvtsi2ss xmm4, rcx; movd eax, xmm4
        assert_eq!(
            bytes_of(
                Operation::FromInteger(IeeeFloatIntegerSource::I64),
                Binary32,
                &["rcx", "rax"]
            ),
            [0xf3, 0x48, 0x0f, 0x2a, 0xe1, 0x66, 0x0f, 0x7e, 0xe0]
        );
        // mov r11d, ecx; cvtsi2sd xmm4, r11; movq r11, xmm4
        assert_eq!(
            bytes_of(
                Operation::FromInteger(IeeeFloatIntegerSource::U32),
                Binary64,
                &["rcx", "r11"]
            ),
            [
                0x41, 0x89, 0xcb, 0xf2, 0x49, 0x0f, 0x2a, 0xe3, 0x66, 0x49, 0x0f, 0x7e, 0xe3,
            ]
        );
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
                    assert!(validate(&physical, other, key(other), operands, &bytes).is_err());
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
