//! Nearest-even scalar IEEE arithmetic on general-register payloads.
//!
//! Scalar graphs keep IEEE values as raw payloads in general registers, so
//! one selected form is four words: `FMOV` both payloads into the fixed
//! scratch registers `v16`/`v17`, one scalar `FADD`/`FSUB`/`FMUL`/`FDIV`
//! into `v16`, and `FMOV` the result payload back. The operation rounds under
//! FPCR, which Omega's native boundaries keep at the canonical control state
//! (round to nearest, ties to even; no trap enables; no flush to zero), so the
//! word computes the exact IEEE-754 operation of its format. It never traps.
//!
//! `v16`/`v17` are caller-saved under AAPCS64 and are never argument homes,
//! so clobbering them needs no callee-save obligation. Both lanes of each are
//! clobbered because a scalar `FMOV` into `d`/`s` clears the upper lane. The
//! operation also accumulates FPSR's sticky exception flags; like every other
//! floating operation it leaves them outside the modeled state, because
//! sticky status is not part of Omega's floating-control invariant
//! (`wiki/spec/build/foreign_bindings.md`, "Floating control state").
//! Encodings follow Arm's A64 scalar floating-point data-processing
//! (two source) and FMOV (general) classes.

use super::{Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint};
use register_model::{RegisterConstraintKey, RegisterUnitId, RegisterViewId};
use selected_instructions::IeeeFloatArithmeticOperation;
use selected_instructions::{
    IeeeFloatArithmeticForm, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedEffects, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
};

/// The fixed scratch vector registers holding the left and right operands.
const LEFT_SCRATCH: u8 = 16;
const RIGHT_SCRATCH: u8 = 17;

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

/// FPCR selects the rounding mode the operation honors.
pub(crate) fn implicit_uses() -> Vec<RegisterUnitId> {
    sorted(view_units("fpcr"))
}

/// Both lanes of both scratch registers.
pub(crate) fn clobbers() -> Vec<RegisterUnitId> {
    sorted(view_units("q16").into_iter().chain(view_units("q17")))
}

pub(crate) fn effects() -> MachineEncodedEffects {
    MachineEncodedEffects {
        implicit_unit_uses: implicit_uses(),
        implicit_unit_clobbers: clobbers(),
        ..MachineEncodedEffects::fallthrough_v1(vec![0, 1], vec![2])
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
            size: MachineSizeKnowledge::ExactBytes(16),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(),
        }],
    }
}

pub(crate) fn footprint(operands: &[RegisterViewId]) -> Aarch64SelectedFormFootprint {
    Aarch64SelectedFormFootprint {
        register_reads: operands[..2].to_vec(),
        register_writes: operands[2..3].to_vec(),
        writes_nzcv: false,
        encoded: effects(),
    }
}

/// The two-source opcode field (bits 15..12) of each operation.
const fn opcode(operation: IeeeFloatArithmeticOperation) -> u32 {
    match operation {
        IeeeFloatArithmeticOperation::Multiply => 0b0000,
        IeeeFloatArithmeticOperation::Divide => 0b0001,
        IeeeFloatArithmeticOperation::Add => 0b0010,
        IeeeFloatArithmeticOperation::Subtract => 0b0011,
    }
}

/// The four canonical words: general-to-float moves, the operation, and the
/// float-to-general move of the result.
fn words(form: IeeeFloatArithmeticForm, registers: &[u8]) -> [u32; 4] {
    let (to_float, to_general, float_type) = if form.is_binary32() {
        (0x1e27_0000_u32, 0x1e26_0000_u32, 0_u32)
    } else {
        (0x9e67_0000, 0x9e66_0000, 1)
    };
    let left = u32::from(registers[0]);
    let right = u32::from(registers[1]);
    let result = u32::from(registers[2]);
    let first = u32::from(LEFT_SCRATCH);
    let second = u32::from(RIGHT_SCRATCH);
    [
        to_float | left << 5 | first,
        to_float | right << 5 | second,
        0x1e20_0800
            | float_type << 22
            | second << 16
            | opcode(form.operation) << 12
            | first << 5
            | first,
        to_general | first << 5 | result,
    ]
}

fn validate_registers(registers: &[u8]) -> Result<(), Aarch64SelectedFormEncodingError> {
    if registers.len() != 3 {
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
    validate_registers(registers)?;
    Ok(words(form, registers)
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

/// Decode every field of the four words independently of `encode`: the move
/// directions and widths, the scratch registers, the operation's type and
/// opcode, and each general register.
pub(crate) fn validate(
    form: IeeeFloatArithmeticForm,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    validate_registers(registers)?;
    let (words, rest) = bytes.as_chunks::<4>();
    let [first, second, operation, last] = words else {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    };
    if !rest.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let [first, second, operation, last] =
        [first, second, operation, last].map(|word| u32::from_le_bytes(*word));
    let field = |word: u32, shift: u32| ((word >> shift) & 31) as u8;
    let (to_float, to_general, float_type) = if form.is_binary32() {
        (0x1e27_0000, 0x1e26_0000, 0)
    } else {
        (0x9e67_0000, 0x9e66_0000, 1)
    };
    let moves_in = first & 0xffff_fc00 == to_float
        && second & 0xffff_fc00 == to_float
        && field(first, 5) == registers[0]
        && field(first, 0) == LEFT_SCRATCH
        && field(second, 5) == registers[1]
        && field(second, 0) == RIGHT_SCRATCH;
    let computes = operation & 0xff20_0c00 == 0x1e20_0800
        && (operation >> 22) & 3 == float_type
        && (operation >> 12) & 15 == opcode(form.operation)
        && field(operation, 16) == RIGHT_SCRATCH
        && field(operation, 5) == LEFT_SCRATCH
        && field(operation, 0) == LEFT_SCRATCH;
    let moves_out = last & 0xffff_fc00 == to_general
        && field(last, 5) == LEFT_SCRATCH
        && field(last, 0) == registers[2];
    if !(moves_in && computes && moves_out) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{encode, validate};
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
    };

    fn form(
        operation: IeeeFloatArithmeticOperation,
        format: IeeeFloatFormat,
    ) -> IeeeFloatArithmeticForm {
        IeeeFloatArithmeticForm { operation, format }
    }

    #[test]
    fn encodings_match_the_architectural_words() {
        // fmov d16, x1; fmov d17, x2; fadd d16, d16, d17; fmov x0, d16
        let add = encode(
            form(IeeeFloatArithmeticOperation::Add, IeeeFloatFormat::Binary64),
            &[1, 2, 0],
        )
        .unwrap();
        let words = add
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect::<Vec<_>>();
        assert_eq!(words, [0x9e67_0030, 0x9e67_0051, 0x1e71_2a10, 0x9e66_0200]);
        // fmov s16, w3; fmov s17, w4; fdiv s16, s16, s17; fmov w5, s16
        let divide = encode(
            form(
                IeeeFloatArithmeticOperation::Divide,
                IeeeFloatFormat::Binary32,
            ),
            &[3, 4, 5],
        )
        .unwrap();
        let words = divide
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect::<Vec<_>>();
        assert_eq!(words, [0x1e27_0070, 0x1e27_0091, 0x1e31_1a10, 0x1e26_0205]);
        let subtract = encode(
            form(
                IeeeFloatArithmeticOperation::Subtract,
                IeeeFloatFormat::Binary64,
            ),
            &[0, 0, 0],
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(subtract[8..12].try_into().unwrap()),
            0x1e71_3a10
        );
        let multiply = encode(
            form(
                IeeeFloatArithmeticOperation::Multiply,
                IeeeFloatFormat::Binary32,
            ),
            &[0, 0, 0],
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(multiply[8..12].try_into().unwrap()),
            0x1e31_0a10
        );
    }

    #[test]
    fn validation_binds_every_bit_operation_format_and_register() {
        for form in IeeeFloatArithmeticForm::ALL {
            let registers = [7, 30, 12];
            let bytes = encode(form, &registers).unwrap();
            validate(form, &registers, &bytes).unwrap();
            for byte_index in 0..bytes.len() {
                for bit in 0..8 {
                    let mut changed = bytes.clone();
                    changed[byte_index] ^= 1 << bit;
                    assert!(validate(form, &registers, &changed).is_err());
                }
            }
            for other in IeeeFloatArithmeticForm::ALL {
                if other != form {
                    assert!(validate(other, &registers, &bytes).is_err());
                }
            }
            assert!(validate(form, &[7, 30, 13], &bytes).is_err());
            assert!(validate(form, &[30, 7, 12], &bytes).is_err());
            assert!(encode(form, &[31, 0, 0]).is_err());
            assert!(validate(form, &registers, &bytes[..12]).is_err());
        }
    }
}
