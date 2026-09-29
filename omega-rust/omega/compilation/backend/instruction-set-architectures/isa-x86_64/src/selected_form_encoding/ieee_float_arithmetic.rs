//! Nearest-even scalar IEEE arithmetic on general-register payloads.
//!
//! Scalar graphs keep IEEE values as raw payloads in general registers, so
//! one selected form is four SSE2 instructions: `MOVD`/`MOVQ` both payloads
//! into the fixed scratch registers `xmm4`/`xmm5`, one scalar
//! `ADDS?`/`SUBS?`/`MULS?`/`DIVS?` into `xmm4`, and `MOVD`/`MOVQ` the result
//! payload back (a 32-bit move zero-extends the general register). SSE2 is
//! the x86-64 baseline. The operation rounds under MXCSR, which Omega's
//! native boundaries keep at the canonical value (round to nearest, ties to
//! even; every exception masked; no flush to zero), so it computes the exact
//! IEEE-754 operation of its format and never traps.
//!
//! `xmm4`/`xmm5` are volatile under both System V and Microsoft x64, so
//! clobbering them needs no callee-save obligation. The operation also
//! accumulates MXCSR's sticky exception flags; those stay outside the modeled
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

/// The fixed scratch registers holding the left and right operands.
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

/// MXCSR selects the rounding mode the operation honors.
pub(crate) fn implicit_uses() -> Vec<RegisterUnitId> {
    sorted(view_units("mxcsr"))
}

pub(crate) fn clobbers() -> Vec<RegisterUnitId> {
    sorted(view_units("xmm4").into_iter().chain(view_units("xmm5")))
}

pub(crate) fn effects() -> MachineEncodedEffects {
    MachineEncodedEffects {
        implicit_unit_uses: implicit_uses(),
        implicit_unit_clobbers: clobbers(),
        ..MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
    }
}

/// Encoded bytes for the given general registers: 4 + 4 + 4 + 4 with no
/// REX.B, one more per move whose general register is r8..r15.
fn size() -> MachineSizeKnowledge {
    MachineSizeKnowledge::EncoderResolved {
        minimum_bytes: 16,
        maximum_bytes: Some(19),
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
            size: size(),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(),
        }],
    }
}

const fn operation_opcode(operation: IeeeFloatArithmeticOperation) -> u8 {
    match operation {
        IeeeFloatArithmeticOperation::Add => 0x58,
        IeeeFloatArithmeticOperation::Multiply => 0x59,
        IeeeFloatArithmeticOperation::Subtract => 0x5c,
        IeeeFloatArithmeticOperation::Divide => 0x5e,
    }
}

/// `66 [REX] 0F 6E|7E /r` between `xmm` (ModRM.reg) and a general register
/// (ModRM.rm). REX.W selects the 64-bit form; REX.B extends the general one.
fn transfer(bytes: &mut Vec<u8>, to_float: bool, wide: bool, xmm: u8, general: u8) {
    bytes.push(0x66);
    let rex = 0x40 | (u8::from(wide) << 3) | (general >> 3);
    if rex != 0x40 {
        bytes.push(rex);
    }
    bytes.extend([
        0x0f,
        if to_float { 0x6e } else { 0x7e },
        0xc0 | (xmm << 3) | (general & 7),
    ]);
}

fn canonical(form: IeeeFloatArithmeticForm, registers: &[u8]) -> Vec<u8> {
    let wide = !form.is_binary32();
    let mut bytes = Vec::with_capacity(19);
    transfer(&mut bytes, true, wide, LEFT_SCRATCH, registers[0]);
    transfer(&mut bytes, true, wide, RIGHT_SCRATCH, registers[1]);
    bytes.extend([
        if wide { 0xf2 } else { 0xf3 },
        0x0f,
        operation_opcode(form.operation),
        0xc0 | (LEFT_SCRATCH << 3) | RIGHT_SCRATCH,
    ]);
    transfer(&mut bytes, false, wide, LEFT_SCRATCH, registers[2]);
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
    if operands.len() != 3 {
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

/// Read one register-to-register transfer and return its direction, width,
/// xmm register and general register.
fn decode_transfer(bytes: &[u8]) -> Option<((bool, bool, u8, u8), &[u8])> {
    let [0x66, tail @ ..] = bytes else {
        return None;
    };
    let (rex, tail) = match tail {
        [rex @ 0x41..=0x4f, rest @ ..] => (*rex, rest),
        _ => (0x40, tail),
    };
    let [0x0f, opcode, modrm, rest @ ..] = tail else {
        return None;
    };
    // REX.R and REX.X would name xmm8+ or an index; neither is canonical here.
    if modrm & 0xc0 != 0xc0 || rex & 6 != 0 {
        return None;
    }
    let to_float = match opcode {
        0x6e => true,
        0x7e => false,
        _ => return None,
    };
    let xmm = (modrm >> 3) & 7;
    let general = (modrm & 7) | ((rex & 1) << 3);
    Some(((to_float, rex & 8 != 0, xmm, general), rest))
}

/// Decode every field of the four instructions independently of `canonical`,
/// then require the canonical prefix choice as well.
pub(crate) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    form: IeeeFloatArithmeticForm,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let registers = registers(physical, form, alternative, operands)?;
    let wide = !form.is_binary32();
    let (left, rest) = decode_transfer(bytes).ok_or(Error::MalformedEncoding)?;
    let (right, rest) = decode_transfer(rest).ok_or(Error::MalformedEncoding)?;
    let [prefix, 0x0f, opcode, modrm, rest @ ..] = rest else {
        return Err(Error::MalformedEncoding);
    };
    let (result, rest) = decode_transfer(rest).ok_or(Error::MalformedEncoding)?;
    if !rest.is_empty()
        || left != (true, wide, LEFT_SCRATCH, registers[0])
        || right != (true, wide, RIGHT_SCRATCH, registers[1])
        || *prefix != if wide { 0xf2 } else { 0xf3 }
        || *opcode != operation_opcode(form.operation)
        || *modrm != 0xc0 | (LEFT_SCRATCH << 3) | RIGHT_SCRATCH
        || result != (false, wide, LEFT_SCRATCH, registers[2])
        || bytes != canonical(form, &registers)
    {
        return Err(Error::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands[..2].to_vec(),
            register_writes: operands[2..].to_vec(),
            writes_rflags: false,
            encoded: effects(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
        MachineAlternativeFamily, MachineAlternativeKey,
    };

    fn key(form: IeeeFloatArithmeticForm) -> MachineAlternativeKey {
        MachineAlternativeKey {
            family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
            variant: 0,
        }
    }

    #[test]
    fn encodings_match_the_architectural_bytes() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let view = |name: &str| physical.model().view_named(name).unwrap().id;
        let add = IeeeFloatArithmeticForm {
            operation: IeeeFloatArithmeticOperation::Add,
            format: IeeeFloatFormat::Binary64,
        };
        // movq xmm4, rcx; movq xmm5, r9; addsd xmm4, xmm5; movq rax, xmm4
        let encoded = encode(
            &physical,
            add,
            key(add),
            &[view("rcx"), view("r9"), view("rax")],
        )
        .unwrap();
        assert_eq!(
            encoded.bytes(),
            [
                0x66, 0x48, 0x0f, 0x6e, 0xe1, 0x66, 0x49, 0x0f, 0x6e, 0xe9, 0xf2, 0x0f, 0x58, 0xe5,
                0x66, 0x48, 0x0f, 0x7e, 0xe0,
            ]
        );
        let divide = IeeeFloatArithmeticForm {
            operation: IeeeFloatArithmeticOperation::Divide,
            format: IeeeFloatFormat::Binary32,
        };
        // movd xmm4, eax; movd xmm5, edx; divss xmm4, xmm5; movd r10d, xmm4
        let encoded = encode(
            &physical,
            divide,
            key(divide),
            &[view("rax"), view("rdx"), view("r10")],
        )
        .unwrap();
        assert_eq!(
            encoded.bytes(),
            [
                0x66, 0x0f, 0x6e, 0xe0, 0x66, 0x0f, 0x6e, 0xea, 0xf3, 0x0f, 0x5e, 0xe5, 0x66, 0x41,
                0x0f, 0x7e, 0xe2,
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
        let operands = [view("rsi"), view("r15"), view("r8")];
        for form in IeeeFloatArithmeticForm::ALL {
            let bytes = encode(&physical, form, key(form), &operands)
                .unwrap()
                .bytes()
                .to_vec();
            for byte_index in 0..bytes.len() {
                for bit in 0..8 {
                    let mut changed = bytes.clone();
                    changed[byte_index] ^= 1 << bit;
                    assert!(validate(&physical, form, key(form), &operands, &changed).is_err());
                }
            }
            for other in IeeeFloatArithmeticForm::ALL {
                if other != form {
                    assert!(validate(&physical, other, key(other), &operands, &bytes).is_err());
                    assert!(validate(&physical, form, key(other), &operands, &bytes).is_err());
                }
            }
            let swapped = [operands[1], operands[0], operands[2]];
            assert!(validate(&physical, form, key(form), &swapped, &bytes).is_err());
            assert!(validate(&physical, form, key(form), &operands, &bytes[1..]).is_err());
        }
    }
}
