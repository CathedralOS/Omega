//! Exact in-place codec access without RCX or implicit scratch registers.
//! High-to-low byte assembly uses constant shifts and sign-extends only the
//! first byte. Admitted codec words select a physical byte count in 1..8.

use super::instruction_bytes::{modrm, rex};
use super::request_validation::resolve_registers;
use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
    X86_64SelectedFormFootprint,
};
use encoded_scalar_plan::{Step, plan};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};

fn codec_plan(load: bool, registers: [u8; 6]) -> Vec<Step> {
    let [_, _, cursor, _, _, _] = registers;
    plan(load, registers, Step::Zero(cursor))
}

fn instruction_size(instruction: Step) -> usize {
    match instruction {
        Step::Zero(_) | Step::Copy(..) | Step::Or(..) | Step::Compare(..) => 3,
        Step::ByteCount(..)
        | Step::SubOne(_)
        | Step::AddOne(_)
        | Step::SignByte(_)
        | Step::ShiftLeft8(_)
        | Step::ShiftRight8(_) => 4,
        Step::LoadByte(..) | Step::ZeroBranch(..) => 9,
        Step::StoreByte(..) => 8,
        Step::TestSigned(..) => 13,
        Step::LessBranch(_) => 6,
        Step::Branch(_) => 5,
    }
}

fn positions(plan: &[Step]) -> Vec<usize> {
    let mut offset = 0;
    plan.iter()
        .map(|instruction| {
            let position = offset;
            offset += instruction_size(*instruction);
            position
        })
        .collect()
}

fn branch_target(offsets: &[usize], target: usize) -> Result<usize, Error> {
    offsets.get(target).copied().ok_or(Error::MalformedEncoding)
}

fn assemble(plan: &[Step]) -> Result<Vec<u8>, Error> {
    let offsets = positions(plan);
    let mut bytes = Vec::new();
    for instruction in plan {
        match *instruction {
            Step::Zero(register) => bytes.extend([
                rex(register, 0, register),
                0x31,
                modrm(3, register, register),
            ]),
            Step::Copy(destination, source) => bytes.extend([
                rex(source, 0, destination),
                0x89,
                modrm(3, source, destination),
            ]),
            Step::ByteCount(destination, source) => bytes.extend([
                rex(destination, 0, source),
                0x0f,
                0xb6,
                modrm(3, destination, source),
            ]),
            Step::SubOne(register) => {
                bytes.extend([rex(0, 0, register), 0x83, modrm(3, 5, register), 1])
            }
            Step::AddOne(register) => {
                bytes.extend([rex(0, 0, register), 0x83, modrm(3, 0, register), 1])
            }
            Step::LoadByte(destination, base, index) => {
                bytes.extend([
                    rex(destination, index, base),
                    0x0f,
                    0xb6,
                    modrm(2, destination, 4),
                    ((index & 7) << 3) | (base & 7),
                ]);
                bytes.extend(0u32.to_le_bytes());
            }
            Step::StoreByte(source, base, index) => {
                bytes.extend([
                    rex(source, index, base) & !8,
                    0x88,
                    modrm(2, source, 4),
                    ((index & 7) << 3) | (base & 7),
                ]);
                bytes.extend(0u32.to_le_bytes());
            }
            Step::SignByte(register) => bytes.extend([
                rex(register, 0, register),
                0x0f,
                0xbe,
                modrm(3, register, register),
            ]),
            Step::ShiftLeft8(register) => {
                bytes.extend([rex(0, 0, register), 0xc1, modrm(3, 4, register), 8])
            }
            Step::ShiftRight8(register) => {
                bytes.extend([rex(0, 0, register), 0xc1, modrm(3, 5, register), 8])
            }
            Step::Or(destination, source) => bytes.extend([
                rex(source, 0, destination),
                0x09,
                modrm(3, source, destination),
            ]),
            Step::Compare(left, right) => {
                bytes.extend([rex(right, 0, left), 0x39, modrm(3, right, left)])
            }
            Step::TestSigned(register, target) => {
                bytes.extend([rex(0, 0, register), 0xf7, modrm(3, 0, register)]);
                bytes.extend(0x100u32.to_le_bytes());
                append_jump(&mut bytes, &[0x0f, 0x84], branch_target(&offsets, target)?)?;
            }
            Step::ZeroBranch(register, target) => {
                bytes.extend([
                    rex(register, 0, register),
                    0x85,
                    modrm(3, register, register),
                ]);
                append_jump(&mut bytes, &[0x0f, 0x84], branch_target(&offsets, target)?)?;
            }
            Step::LessBranch(target) => {
                append_jump(&mut bytes, &[0x0f, 0x82], branch_target(&offsets, target)?)?
            }
            Step::Branch(target) => {
                append_jump(&mut bytes, &[0xe9], branch_target(&offsets, target)?)?
            }
        }
    }
    Ok(bytes)
}

fn append_jump(bytes: &mut Vec<u8>, prefix: &[u8], target: usize) -> Result<(), Error> {
    bytes.extend(prefix);
    let displacement = i32::try_from(target as i64 - (bytes.len() as i64 + 4))
        .map_err(|_| Error::MalformedEncoding)?;
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn decode_one(bytes: &[u8], position: usize) -> Result<(Step, usize), Error> {
    let instruction = bytes.get(position..).ok_or(Error::MalformedEncoding)?;
    let fail = Error::EncodedFormMismatch;
    let fields = |prefix: u8, mode: u8| {
        (
            ((mode >> 3) & 7) | ((prefix & 4) << 1),
            (mode & 7) | ((prefix & 1) << 3),
        )
    };
    let target = |offset: usize, width: usize| -> Result<usize, Error> {
        let displacement: [u8; 4] = instruction
            .get(offset..offset + 4)
            .ok_or(Error::MalformedEncoding)?
            .try_into()
            .map_err(|_| Error::MalformedEncoding)?;
        usize::try_from(
            position as i64 + width as i64 + i64::from(i32::from_le_bytes(displacement)),
        )
        .map_err(|_| Error::MalformedEncoding)
    };
    match *instruction {
        [prefix, 0x31, mode, ..] if prefix & 0xfa == 0x48 && mode >> 6 == 3 => {
            let (source, destination) = fields(prefix, mode);
            if source == destination {
                Ok((Step::Zero(source), 3))
            } else {
                Err(fail)
            }
        }
        [prefix, opcode @ (0x89 | 0x09 | 0x39), mode, ..]
            if prefix & 0xfa == 0x48 && mode >> 6 == 3 =>
        {
            let (source, destination) = fields(prefix, mode);
            Ok((
                match opcode {
                    0x89 => Step::Copy(destination, source),
                    0x09 => Step::Or(destination, source),
                    _ => Step::Compare(destination, source),
                },
                3,
            ))
        }
        [prefix, 0x0f, opcode @ (0xb6 | 0xbe), mode, ..]
            if prefix & 0xfa == 0x48 && mode >> 6 == 3 =>
        {
            let (destination, source) = fields(prefix, mode);
            if opcode == 0xb6 {
                Ok((Step::ByteCount(destination, source), 4))
            } else if source == destination {
                Ok((Step::SignByte(source), 4))
            } else {
                Err(fail)
            }
        }
        [prefix, 0x83, mode, 1, ..] if prefix & 0xfe == 0x48 && mode >> 6 == 3 => {
            let register = (mode & 7) | ((prefix & 1) << 3);
            match (mode >> 3) & 7 {
                0 => Ok((Step::AddOne(register), 4)),
                5 => Ok((Step::SubOne(register), 4)),
                _ => Err(fail),
            }
        }
        [prefix, 0xc1, mode, 8, ..] if prefix & 0xfe == 0x48 && mode >> 6 == 3 => {
            let register = (mode & 7) | ((prefix & 1) << 3);
            match (mode >> 3) & 7 {
                4 => Ok((Step::ShiftLeft8(register), 4)),
                5 => Ok((Step::ShiftRight8(register), 4)),
                _ => Err(fail),
            }
        }
        [prefix, 0x0f, 0xb6, mode, sib, 0, 0, 0, 0, ..]
            if prefix & 0xf8 == 0x48 && mode & 0xc7 == 0x84 && sib >> 6 == 0 =>
        {
            let destination = ((mode >> 3) & 7) | ((prefix & 4) << 1);
            let base = (sib & 7) | ((prefix & 1) << 3);
            let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
            if index == 4 {
                Err(fail)
            } else {
                Ok((Step::LoadByte(destination, base, index), 9))
            }
        }
        [prefix, 0x88, mode, sib, 0, 0, 0, 0, ..]
            if prefix & 0xf8 == 0x40 && mode & 0xc7 == 0x84 && sib >> 6 == 0 =>
        {
            let source = ((mode >> 3) & 7) | ((prefix & 4) << 1);
            let base = (sib & 7) | ((prefix & 1) << 3);
            let index = ((sib >> 3) & 7) | ((prefix & 2) << 2);
            if index == 4 {
                Err(fail)
            } else {
                Ok((Step::StoreByte(source, base, index), 8))
            }
        }
        [prefix, 0xf7, mode, 0, 1, 0, 0, 0x0f, 0x84, _, _, _, _, ..]
            if prefix & 0xfe == 0x48 && mode & 0xf8 == 0xc0 =>
        {
            Ok((
                Step::TestSigned((mode & 7) | ((prefix & 1) << 3), target(9, 13)?),
                13,
            ))
        }
        [prefix, 0x85, mode, 0x0f, 0x84, _, _, _, _, ..]
            if prefix & 0xfa == 0x48 && mode >> 6 == 3 =>
        {
            let (source, destination) = fields(prefix, mode);
            if source == destination {
                Ok((Step::ZeroBranch(source, target(5, 9)?), 9))
            } else {
                Err(fail)
            }
        }
        [0x0f, 0x82, _, _, _, _, ..] => Ok((Step::LessBranch(target(2, 6)?), 6)),
        [0xe9, _, _, _, _, ..] => Ok((Step::Branch(target(1, 5)?), 5)),
        _ => Err(fail),
    }
}

fn resolved_plan(plan: &[Step]) -> Result<Vec<Step>, Error> {
    let offsets = positions(plan);
    plan.iter()
        .map(|instruction| {
            Ok(match *instruction {
                Step::TestSigned(register, target) => {
                    Step::TestSigned(register, branch_target(&offsets, target)?)
                }
                Step::ZeroBranch(register, target) => {
                    Step::ZeroBranch(register, branch_target(&offsets, target)?)
                }
                Step::LessBranch(target) => Step::LessBranch(branch_target(&offsets, target)?),
                Step::Branch(target) => Step::Branch(branch_target(&offsets, target)?),
                other => other,
            })
        })
        .collect()
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(bool, [u8; 6]), Error> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(Error::NonCanonicalPhysicalModel);
    }
    let load = kind == SelectedInstructionKind::LoadEncodedScalar;
    let family = if load {
        MachineAlternativeFamily::LoadEncodedScalar
    } else {
        MachineAlternativeFamily::StoreEncodedScalar
    };
    if !matches!(
        kind,
        SelectedInstructionKind::LoadEncodedScalar | SelectedInstructionKind::StoreEncodedScalar
    ) || alternative != (MachineAlternativeKey { family, variant: 0 })
    {
        return Err(Error::EncodedFormMismatch);
    }
    let registers: [u8; 6] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| Error::OperandCountMismatch)?;
    let inputs = if load { 2 } else { 3 };
    for scratch in inputs..6 {
        if registers[..scratch].contains(&registers[scratch]) {
            return Err(Error::EncodedFormMismatch);
        }
    }
    Ok((load, registers))
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let (load, registers) = request(physical, kind, alternative, operands)?;
    let bytes = assemble(&codec_plan(load, registers))?;
    validate(physical, kind, alternative, operands, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let (load, registers) = request(physical, kind, alternative, operands)?;
    let mut decoded = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let (instruction, size) = decode_one(bytes, position)?;
        decoded.push(instruction);
        position += size;
    }
    if decoded != resolved_plan(&codec_plan(load, registers))? {
        return Err(Error::EncodedFormMismatch);
    }
    let inputs = if load { 2 } else { 3 };
    let flags = physical
        .model()
        .view_named("rflags")
        .ok_or(Error::NonCanonicalPhysicalModel)?
        .units
        .clone();
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands[..inputs].to_vec(),
            register_writes: operands[inputs..].to_vec(),
            writes_rflags: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: (0..inputs as u16).collect(),
                external_operand_writes: (inputs as u16..6).collect(),
                implicit_unit_uses: vec![],
                implicit_unit_defs: vec![],
                implicit_unit_clobbers: flags,
                memory: if load {
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
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoded_scalar_plan::replay;

    fn operands(physical: &ValidatedPhysicalRegisterModel) -> [RegisterViewId; 6] {
        ["r8", "r9", "r10", "r11", "r12", "r13"]
            .map(|name| physical.model().view_named(name).unwrap().id)
    }

    fn execute(bytes: &[u8], state: &mut [u64; 32], memory: &mut [u8]) -> Vec<usize> {
        replay::execute(
            bytes,
            bytes.len(),
            |bytes, position| decode_one(bytes, position).unwrap(),
            state,
            memory,
        )
    }

    #[test]
    fn encoded_scalar_access_has_exact_footprint_and_extension() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let operands = operands(&physical);
        let registers: [u8; 6] = resolve_registers(&physical, &operands)
            .unwrap()
            .try_into()
            .unwrap();
        let codecs = (1..=8u64)
            .flat_map(|width| [width, width | 256])
            .chain([516, 520, 1025, 2056]);
        for codec in codecs {
            let width = codec & 255;
            let signed = codec & 256 != 0;
            let payload = if codec == 1025 {
                1
            } else {
                0x807f_6655_4433_2280u64
            };
            let mut memory = [85u8; 10];
            let mut state = [0u64; 32];
            state[registers[0] as usize] = 1;
            state[registers[1] as usize] = codec;
            state[registers[2] as usize] = payload;
            let original = state;
            let store = encode(
                &physical,
                SelectedInstructionKind::StoreEncodedScalar,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::StoreEncodedScalar,
                    variant: 0,
                },
                &operands,
            )
            .unwrap();
            let writes = execute(&store.bytes, &mut state, &mut memory);
            assert_eq!(writes, (1..=width as usize).collect::<Vec<_>>());
            assert_eq!(memory[0], 85);
            assert_eq!(memory[width as usize + 1], 85);
            assert_eq!(
                &memory[1..=width as usize],
                &payload.to_le_bytes()[..width as usize]
            );
            for input in 0..3 {
                assert_eq!(
                    state[registers[input] as usize],
                    original[registers[input] as usize]
                );
            }
            let load = encode(
                &physical,
                SelectedInstructionKind::LoadEncodedScalar,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::LoadEncodedScalar,
                    variant: 0,
                },
                &operands,
            )
            .unwrap();
            let reads = execute(&load.bytes, &mut state, &mut memory);
            assert_eq!(reads, (1..=width as usize).rev().collect::<Vec<_>>());
            let shift = 64 - width * 8;
            let expected = if signed {
                ((payload << shift) as i64 >> shift) as u64
            } else {
                (payload << shift) >> shift
            };
            assert_eq!(state[registers[5] as usize], expected);
            for input in 0..2 {
                assert_eq!(
                    state[registers[input] as usize],
                    original[registers[input] as usize]
                );
            }
        }
    }

    #[test]
    fn encoded_scalar_stream_rejects_every_single_bit_mutation() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let operands = operands(&physical);
        for (kind, family) in [
            (
                SelectedInstructionKind::LoadEncodedScalar,
                MachineAlternativeFamily::LoadEncodedScalar,
            ),
            (
                SelectedInstructionKind::StoreEncodedScalar,
                MachineAlternativeFamily::StoreEncodedScalar,
            ),
        ] {
            let alternative = MachineAlternativeKey { family, variant: 0 };
            let encoded = encode(&physical, kind, alternative, &operands).unwrap();
            for bit in 0..encoded.bytes.len() * 8 {
                let mut changed = encoded.bytes.clone();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate(&physical, kind, alternative, &operands, &changed).is_err(),
                    "{kind:?} bit {bit}"
                );
            }
            let input_count = if kind == SelectedInstructionKind::LoadEncodedScalar {
                2
            } else {
                3
            };
            for scratch in input_count..6 {
                for prior in 0..scratch {
                    let mut aliased = operands;
                    aliased[scratch] = operands[prior];
                    assert!(
                        encode(&physical, kind, alternative, &aliased).is_err(),
                        "{kind:?} scratch {scratch} aliases operand {prior}"
                    );
                    assert!(
                        validate(&physical, kind, alternative, &aliased, &encoded.bytes).is_err()
                    );
                }
            }
            for input in 1..input_count {
                let mut shared_input = operands;
                shared_input[input] = operands[0];
                let encoded = encode(&physical, kind, alternative, &shared_input).unwrap();
                assert!(
                    validate(&physical, kind, alternative, &shared_input, &encoded.bytes).is_ok()
                );
            }
        }
    }
}
