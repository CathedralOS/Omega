//! Exact in-place scalar codec access. Descending loads seed the high byte
//! before constant shifts, so signed extension needs no variable-shift home.
//! Codec words are admitted by the descriptor producer; byte counts are 1..8.

use super::selected_forms::request_registers;
use super::{Aarch64SelectedFormEncodingError as Error, ValidatedAarch64SelectedFormEncoding};
use encoded_scalar_plan::{Step, plan};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
    SelectedInstructionKind,
};

fn codec_plan(load: bool, registers: [u8; 6]) -> Vec<Step> {
    let [_, _, cursor, count, _, _] = registers;
    plan(load, registers, Step::Compare(cursor, count))
}

fn encode_word(word: Step, position: usize) -> Result<u32, Error> {
    let relative = |target: usize, bits: u32| -> Result<u32, Error> {
        let displacement = target as i64 - position as i64;
        let bound = 1i64 << (bits - 1);
        if !(-bound..bound).contains(&displacement) {
            return Err(Error::MalformedEncoding);
        }
        Ok((displacement as u32) & ((1 << bits) - 1))
    };
    Ok(match word {
        Step::Zero(destination) => 0xd280_0000 | u32::from(destination),
        Step::Copy(destination, source) => {
            0xaa00_03e0 | (u32::from(source) << 16) | u32::from(destination)
        }
        Step::ByteCount(destination, source) => {
            0xd340_1c00 | (u32::from(source) << 5) | u32::from(destination)
        }
        Step::SubOne(register) => 0xd100_0400 | (u32::from(register) << 5) | u32::from(register),
        Step::AddOne(register) => 0x9100_0400 | (u32::from(register) << 5) | u32::from(register),
        Step::LoadByte(destination, base, index) => {
            0x3860_6800 | (u32::from(index) << 16) | (u32::from(base) << 5) | u32::from(destination)
        }
        Step::StoreByte(source, base, index) => {
            0x3820_6800 | (u32::from(index) << 16) | (u32::from(base) << 5) | u32::from(source)
        }
        Step::SignByte(register) => 0x9340_1c00 | (u32::from(register) << 5) | u32::from(register),
        Step::ShiftLeft8(register) => {
            0xd378_dc00 | (u32::from(register) << 5) | u32::from(register)
        }
        Step::ShiftRight8(register) => {
            0xd348_fc00 | (u32::from(register) << 5) | u32::from(register)
        }
        Step::Or(destination, source) => {
            0xaa00_0000
                | (u32::from(source) << 16)
                | (u32::from(destination) << 5)
                | u32::from(destination)
        }
        Step::Compare(left, right) => {
            0xeb00_001f | (u32::from(right) << 16) | (u32::from(left) << 5)
        }
        Step::TestSigned(register, target) => {
            0x3640_0000 | (relative(target, 14)? << 5) | u32::from(register)
        }
        Step::ZeroBranch(register, target) => {
            0xb400_0000 | (relative(target, 19)? << 5) | u32::from(register)
        }
        Step::LessBranch(target) => 0x5400_0003 | (relative(target, 19)? << 5),
        Step::Branch(target) => 0x1400_0000 | relative(target, 26)?,
    })
}

fn decode_word(encoded: u32, position: usize) -> Result<Step, Error> {
    let destination = (encoded & 31) as u8;
    let source = ((encoded >> 5) & 31) as u8;
    let other = ((encoded >> 16) & 31) as u8;
    let target = |value: u32, bits: u32| -> Result<usize, Error> {
        let displacement = ((value << (32 - bits)) as i32) >> (32 - bits);
        usize::try_from(position as i64 + i64::from(displacement))
            .map_err(|_| Error::MalformedEncoding)
    };
    if encoded & 0xffff_ffe0 == 0xd280_0000 {
        return Ok(Step::Zero(destination));
    }
    if encoded & 0xffe0_ffe0 == 0xaa00_03e0 {
        return Ok(Step::Copy(destination, other));
    }
    if encoded & 0xffff_fc00 == 0xd340_1c00 {
        return Ok(Step::ByteCount(destination, source));
    }
    if encoded & 0xffff_fc00 == 0xd100_0400 && destination == source {
        return Ok(Step::SubOne(destination));
    }
    if encoded & 0xffff_fc00 == 0x9100_0400 && destination == source {
        return Ok(Step::AddOne(destination));
    }
    if encoded & 0xffe0_fc00 == 0x3860_6800 {
        return Ok(Step::LoadByte(destination, source, other));
    }
    if encoded & 0xffe0_fc00 == 0x3820_6800 {
        return Ok(Step::StoreByte(destination, source, other));
    }
    if destination == source {
        match encoded & 0xffff_fc00 {
            0x9340_1c00 => return Ok(Step::SignByte(destination)),
            0xd378_dc00 => return Ok(Step::ShiftLeft8(destination)),
            0xd348_fc00 => return Ok(Step::ShiftRight8(destination)),
            _ => {}
        }
    }
    if encoded & 0xffe0_fc00 == 0xaa00_0000 && destination == source {
        return Ok(Step::Or(destination, other));
    }
    if encoded & 0xffe0_fc1f == 0xeb00_001f {
        return Ok(Step::Compare(source, other));
    }
    if encoded & 0xfff8_0000 == 0x3640_0000 {
        return Ok(Step::TestSigned(
            destination,
            target((encoded >> 5) & 0x3fff, 14)?,
        ));
    }
    if encoded & 0xff00_0000 == 0xb400_0000 {
        return Ok(Step::ZeroBranch(
            destination,
            target((encoded >> 5) & 0x7ffff, 19)?,
        ));
    }
    if encoded & 0xff00_001f == 0x5400_0003 {
        return Ok(Step::LessBranch(target((encoded >> 5) & 0x7ffff, 19)?));
    }
    if encoded & 0xfc00_0000 == 0x1400_0000 {
        return Ok(Step::Branch(target(encoded & 0x03ff_ffff, 26)?));
    }
    Err(Error::MalformedEncoding)
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(bool, [u8; 6]), Error> {
    let load = kind == SelectedInstructionKind::LoadEncodedScalar;
    let family = if load {
        MachineAlternativeFamily::LoadEncodedScalar
    } else {
        MachineAlternativeFamily::StoreEncodedScalar
    };
    if !matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) {
        return Err(Error::AlternativeMismatch);
    }
    let inputs = if load { 2 } else { 3 };
    let registers: [u8; 6] = request_registers(physical, alternative, family, operands, inputs)?;
    Ok((load, registers))
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Error> {
    let (load, registers) = request(physical, kind, alternative, operands)?;
    let mut bytes = Vec::new();
    for (position, word) in codec_plan(load, registers).into_iter().enumerate() {
        bytes.extend(encode_word(word, position)?.to_le_bytes());
    }
    validate(physical, kind, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Error> {
    let (load, registers) = request(physical, kind, alternative, operands)?;
    let (words, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return Err(Error::MalformedEncoding);
    }
    let decoded = words
        .iter()
        .enumerate()
        .map(|(position, word)| decode_word(u32::from_le_bytes(*word), position))
        .collect::<Result<Vec<_>, _>>()?;
    if decoded != codec_plan(load, registers) {
        return Err(Error::EncodedFormMismatch);
    }
    let inputs = if load { 2 } else { 3 };
    super::nzcv_effects_encoding(
        physical,
        operands,
        bytes,
        inputs,
        if load {
            MachineEncodedMemoryEffect::ReadEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            }
        } else {
            MachineEncodedMemoryEffect::WriteEncodedScalarV1 {
                pointer_operand: 0,
                codec_operand: 1,
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selected_form_encoding::selected_forms::resolve_registers;
    use encoded_scalar_plan::{harness, replay};

    fn form() -> (ValidatedPhysicalRegisterModel, [RegisterViewId; 6]) {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let operands = ["x0", "x1", "x2", "x3", "x4", "x5"]
            .map(|name| physical.model().view_named(name).unwrap().id);
        (physical, operands)
    }

    fn execute(bytes: &[u8], state: &mut [u64; 32], memory: &mut [u8]) -> Vec<usize> {
        replay::execute(
            bytes,
            bytes.len() / 4,
            |bytes, position| {
                let word: [u8; 4] = bytes[position * 4..position * 4 + 4].try_into().unwrap();
                (decode_word(u32::from_le_bytes(word), position).unwrap(), 1)
            },
            state,
            memory,
        )
    }

    #[test]
    fn encoded_scalar_access_has_exact_footprint_and_extension() {
        let (physical, operands) = form();
        let registers: [u8; 6] = resolve_registers(&physical, &operands)
            .unwrap()
            .try_into()
            .unwrap();
        harness::exact_footprint_and_extension(
            registers,
            operands,
            |kind, alternative, operands| {
                encode(&physical, kind, alternative, operands).map(|encoded| encoded.bytes)
            },
            execute,
        );
    }

    #[test]
    fn encoded_scalar_stream_rejects_every_single_bit_mutation() {
        let (physical, operands) = form();
        harness::rejects_every_single_bit_mutation(
            operands,
            |kind, alternative, operands| {
                encode(&physical, kind, alternative, operands).map(|encoded| encoded.bytes)
            },
            |kind, alternative, operands, bytes| {
                validate(&physical, kind, alternative, operands, bytes).is_err()
            },
        );
    }
}
