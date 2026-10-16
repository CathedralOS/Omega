//! HLT and scalar IN/OUT and RDMSR/WRMSR envelopes. Inputs are snapshotted before
//! architectural registers change; R10/R11 cycles use the already-clobbered
//! RAX. BSWAP and LEA split/join EDX:EAX without changing flags or the stack.
//! Intel SDM volume 2 specifies RDMSR's zero-extended EAX/EDX result halves:
//! https://cdrdv2-public.intel.com/835757/325383-sdm-vol-2abcd.pdf
//! The validator decodes and symbolically follows bits independently of byte
//! construction. It checks operand roles, observed results and untouched state;
//! none of these privileged instructions is executed by hosted tests.

use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError as Error,
    X86_64SelectedFormFootprint,
};
use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily as Family,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedEffects, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind as Semantic,
    MachineSizeKnowledge, MachineTrapBehavior, RegisterOperation as Operation,
    SelectedInstructionKind,
};

pub(crate) fn semantic(operation: Operation) -> Option<Semantic> {
    Some(match operation {
        Operation::PortRead => Semantic::PortRegisterRead,
        Operation::PortWrite => Semantic::PortRegisterWrite,
        Operation::ModelSpecificRead => Semantic::ModelSpecificRead,
        Operation::ModelSpecificWrite => Semantic::ModelSpecificWrite,
        Operation::ProcessorHalt => Semantic::ProcessorHalt,
        _ => return None,
    })
}

pub(crate) fn operation(semantic: Semantic) -> Option<Operation> {
    Some(match semantic {
        Semantic::PortRegisterRead => Operation::PortRead,
        Semantic::PortRegisterWrite => Operation::PortWrite,
        Semantic::ModelSpecificRead => Operation::ModelSpecificRead,
        Semantic::ModelSpecificWrite => Operation::ModelSpecificWrite,
        Semantic::ProcessorHalt => Operation::ProcessorHalt,
        _ => return None,
    })
}

pub(crate) fn family(operation: Operation) -> Result<Family, Error> {
    Ok(Family::from(
        semantic(operation).ok_or(Error::AlternativeMismatch)?,
    ))
}

fn scratch(operation: Operation) -> &'static [&'static str] {
    match operation {
        Operation::PortRead => &["rax", "rdx", "r10"],
        Operation::PortWrite => &["rax", "rdx", "r10", "r11"],
        Operation::ModelSpecificRead | Operation::ModelSpecificWrite => {
            &["rax", "rcx", "rdx", "r10", "r11"]
        }
        _ => &[],
    }
}

pub(crate) fn effects(operation: Operation) -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model_ref();
    let mut clobbers = scratch(operation)
        .iter()
        .flat_map(|name| {
            physical
                .view_named(name)
                .expect("canonical x86 scratch")
                .units
                .iter()
                .copied()
        })
        .collect::<Vec<_>>();
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        implicit_unit_clobbers: clobbers,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ..MachineEncodedEffects::fallthrough_v1(
            (0..operation.input_carriers().len() as u16).collect(),
            if operation.produces_value() {
                vec![operation.input_carriers().len() as u16]
            } else {
                vec![]
            },
        )
    }
}

pub(crate) fn declaration(
    operation: Operation,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: semantic(operation).expect("x86 register operation"),
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: family(operation).expect("x86 register family"),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: if operation == Operation::ProcessorHalt {
                MachineSizeKnowledge::ExactBytes(1)
            } else {
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: 1,
                    maximum_bytes: Some(40),
                }
            },
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(operation),
        }],
    }
}

fn move_register(bytes: &mut Vec<u8>, source: u8, destination: u8, wide: bool) {
    bytes.extend([
        0x40 | (u8::from(wide) << 3) | ((source >> 3) << 2) | (destination >> 3),
        0x89,
        0xc0 | ((source & 7) << 3) | (destination & 7),
    ]);
}
fn swap_bytes(bytes: &mut Vec<u8>, register: u8, wide: bool) {
    let rex = 0x40 | (u8::from(wide) << 3) | (register >> 3);
    if rex != 0x40 {
        bytes.push(rex);
    }
    bytes.extend([0x0f, 0xc8 | (register & 7)]);
}

pub(crate) fn encode(operation: Operation, registers: &[u8]) -> Result<Vec<u8>, Error> {
    family(operation)?;
    if operation == Operation::ProcessorHalt {
        return if registers.is_empty() {
            Ok(vec![0xf4])
        } else {
            Err(Error::OperandCountMismatch)
        };
    }
    let [first, second] = registers else {
        return Err(Error::OperandCountMismatch);
    };
    if registers.iter().any(|register| *register > 15)
        || (operation.produces_value() && *second == 4)
    {
        return Err(Error::EncodedFormMismatch);
    }
    let mut bytes = Vec::new();
    if !operation.produces_value() && *second == 10 && *first != 10 {
        if *first == 11 {
            move_register(&mut bytes, 11, 0, true);
            move_register(&mut bytes, 10, 11, true);
            move_register(&mut bytes, 0, 10, true);
        } else {
            move_register(&mut bytes, 10, 11, true);
            move_register(&mut bytes, *first, 10, true);
        }
    } else {
        move_register(&mut bytes, *first, 10, true);
        if !operation.produces_value() {
            move_register(&mut bytes, *second, 11, true);
        }
    }
    match operation {
        Operation::PortRead => {
            move_register(&mut bytes, 10, 2, false);
            bytes.push(0xec);
            bytes.extend([
                0x48 | ((*second >> 3) << 2),
                0x0f,
                0xb6,
                0xc0 | ((*second & 7) << 3),
            ]);
        }
        Operation::PortWrite => {
            move_register(&mut bytes, 10, 2, false);
            move_register(&mut bytes, 11, 0, false);
            bytes.push(0xee);
        }
        Operation::ModelSpecificRead => {
            move_register(&mut bytes, 10, 1, false);
            bytes.extend([0x0f, 0x32]);
            swap_bytes(&mut bytes, 2, false);
            swap_bytes(&mut bytes, 2, true);
            // LEA result,[RAX+RDX]; halves occupy disjoint bits.
            bytes.extend([
                0x48 | ((*second >> 3) << 2),
                0x8d,
                0x04 | ((*second & 7) << 3),
                0x10,
            ]);
        }
        Operation::ModelSpecificWrite => {
            move_register(&mut bytes, 10, 1, false);
            move_register(&mut bytes, 11, 2, true);
            swap_bytes(&mut bytes, 2, true);
            swap_bytes(&mut bytes, 2, false);
            move_register(&mut bytes, 11, 0, false);
            bytes.extend([0x0f, 0x30]);
        }
        _ => return Err(Error::AlternativeMismatch),
    }
    Ok(bytes)
}

#[derive(Clone, Copy)]
enum Decoded {
    Move {
        source: u8,
        destination: u8,
        bits: usize,
    },
    Swap {
        register: u8,
        bytes: usize,
    },
    Join {
        destination: u8,
    },
    Physical(Operation),
}

fn decode(bytes: &[u8]) -> Option<Vec<Decoded>> {
    let mut cursor = 0;
    let mut decoded = Vec::new();
    while cursor < bytes.len() {
        match bytes[cursor] {
            0xf4 => {
                decoded.push(Decoded::Physical(Operation::ProcessorHalt));
                cursor += 1;
                continue;
            }
            0xec | 0xee => {
                decoded.push(Decoded::Physical(if bytes[cursor] == 0xec {
                    Operation::PortRead
                } else {
                    Operation::PortWrite
                }));
                cursor += 1;
                continue;
            }
            0x0f => {
                decoded.push(match *bytes.get(cursor + 1)? {
                    0x32 => Decoded::Physical(Operation::ModelSpecificRead),
                    0x30 => Decoded::Physical(Operation::ModelSpecificWrite),
                    opcode @ 0xc8..=0xcf => Decoded::Swap {
                        register: opcode & 7,
                        bytes: 4,
                    },
                    _ => return None,
                });
                cursor += 2;
                continue;
            }
            0x40..=0x4f => {}
            _ => return None,
        }
        let rex = bytes[cursor];
        let opcode = *bytes.get(cursor + 1)?;
        let operand = *bytes.get(cursor + 2)?;
        match opcode {
            0x89 if operand >> 6 == 3 && rex & 2 == 0 => {
                decoded.push(Decoded::Move {
                    source: ((operand >> 3) & 7) | ((rex & 4) << 1),
                    destination: (operand & 7) | ((rex & 1) << 3),
                    bits: if rex & 8 != 0 { 64 } else { 32 },
                });
                cursor += 3;
            }
            0x0f if operand & 0xf8 == 0xc8 && rex & 6 == 0 => {
                decoded.push(Decoded::Swap {
                    register: (operand & 7) | ((rex & 1) << 3),
                    bytes: if rex & 8 != 0 { 8 } else { 4 },
                });
                cursor += 3;
            }
            0x0f if operand == 0xb6 && rex & 0xb == 8 => {
                let addressing = *bytes.get(cursor + 3)?;
                if addressing & 0xc7 != 0xc0 {
                    return None;
                }
                decoded.push(Decoded::Move {
                    source: 0,
                    destination: ((addressing >> 3) & 7) | ((rex & 4) << 1),
                    bits: 8,
                });
                cursor += 4;
            }
            0x8d if operand & 0xc7 == 4 && rex & 0xb == 8 && *bytes.get(cursor + 3)? == 0x10 => {
                decoded.push(Decoded::Join {
                    destination: ((operand >> 3) & 7) | ((rex & 4) << 1),
                });
                cursor += 4;
            }
            _ => return None,
        }
    }
    Some(decoded)
}

// Every input register bit starts with a distinct identity; zero and the fresh
// hardware result have separate identities. Aliased input operands therefore
// retain the exact same value instead of inventing independent source words.
fn validate_bits(operation: Operation, registers: &[u8], bytes: &[u8]) -> Option<()> {
    if operation == Operation::ProcessorHalt {
        return (registers.is_empty()
            && matches!(
                decode(bytes)?.as_slice(),
                [Decoded::Physical(Operation::ProcessorHalt)]
            ))
        .then_some(());
    }
    const ZERO: u16 = u16::MAX;
    const RESULT: u16 = 1024;
    let mut words = std::array::from_fn::<_, 16, _>(|register| {
        std::array::from_fn::<_, 64, _>(|bit| (register * 64 + bit) as u16)
    });
    let initial = words;
    let first = *registers.first()? as usize;
    let second = *registers.get(1)? as usize;
    if first >= 16 || second >= 16 || (operation.produces_value() && second == 4) {
        return None;
    }
    let mut physical_count = 0;
    for instruction in decode(bytes)? {
        match instruction {
            Decoded::Move {
                source,
                destination,
                bits,
            } => {
                let source = words[source as usize];
                words[destination as usize] = [ZERO; 64];
                words[destination as usize][..bits].copy_from_slice(&source[..bits]);
            }
            Decoded::Swap { register, bytes } => {
                let source = words[register as usize];
                words[register as usize] = [ZERO; 64];
                for byte in 0..bytes {
                    words[register as usize][byte * 8..byte * 8 + 8]
                        .copy_from_slice(&source[(bytes - 1 - byte) * 8..(bytes - byte) * 8]);
                }
            }
            Decoded::Join { destination } => {
                if words[0][32..].iter().any(|bit| *bit != ZERO)
                    || words[2][..32].iter().any(|bit| *bit != ZERO)
                {
                    return None;
                }
                let mut result = words[0];
                result[32..].copy_from_slice(&words[2][32..]);
                words[destination as usize] = result;
            }
            Decoded::Physical(actual) => {
                if actual != operation {
                    return None;
                }
                physical_count += 1;
                match actual {
                    Operation::PortRead | Operation::PortWrite => {
                        if words[2][..16] != initial[first][..16] {
                            return None;
                        }
                        if actual == Operation::PortWrite {
                            if words[0][..8] != initial[second][..8] {
                                return None;
                            }
                        } else {
                            for (bit, origin) in words[0][..8].iter_mut().enumerate() {
                                *origin = RESULT + bit as u16;
                            }
                        }
                    }
                    Operation::ModelSpecificRead | Operation::ModelSpecificWrite => {
                        if words[1][..32] != initial[first][..32] {
                            return None;
                        }
                        if actual == Operation::ModelSpecificWrite {
                            if words[0][..32] != initial[second][..32]
                                || words[2][..32] != initial[second][32..]
                            {
                                return None;
                            }
                        } else {
                            words[0] = [ZERO; 64];
                            words[2] = [ZERO; 64];
                            for bit in 0..32 {
                                words[0][bit] = RESULT + bit as u16;
                                words[2][bit] = RESULT + 32 + bit as u16;
                            }
                        }
                    }
                    _ => return None,
                }
            }
        }
    }
    if physical_count != 1 {
        return None;
    }
    if let Some(carrier) = operation.result_carrier() {
        let bits = carrier.bits() as usize;
        for (bit, origin) in words[second].iter().enumerate() {
            if *origin
                != if bit < bits {
                    RESULT + bit as u16
                } else {
                    ZERO
                }
            {
                return None;
            }
        }
    }
    let clobbered = |register: usize| match operation {
        Operation::PortRead => [0, 2, 10].contains(&register),
        Operation::PortWrite => [0, 2, 10, 11].contains(&register),
        _ => [0, 1, 2, 10, 11].contains(&register),
    };
    for register in 0..16 {
        if !clobbered(register)
            && !(operation.produces_value() && register == second)
            && words[register] != initial[register]
        {
            return None;
        }
    }
    Some(())
}

pub(crate) fn footprint(
    operation: Operation,
    operands: &[RegisterViewId],
) -> X86_64SelectedFormFootprint {
    X86_64SelectedFormFootprint {
        register_reads: if operation.produces_value() {
            vec![operands[0]]
        } else {
            operands.to_vec()
        },
        register_writes: if operation.produces_value() {
            vec![operands[1]]
        } else {
            vec![]
        },
        writes_rflags: false,
        encoded: effects(operation),
    }
}

pub(crate) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    operation: Operation,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, Error> {
    let kind = SelectedInstructionKind::RegisterAccess { operation };
    super::request_validation::validate_request(physical, kind, alternative, operands)?;
    let registers = super::request_validation::resolve_scalar_registers(physical, kind, operands)?;
    validate_bits(operation, &registers, bytes).ok_or(Error::EncodedFormMismatch)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: footprint(operation, operands),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processor_halt_is_one_operand_free_hardware_event() {
        let operation = Operation::ProcessorHalt;
        assert_eq!(encode(operation, &[]).unwrap(), [0xf4]);
        assert!(encode(operation, &[0]).is_err());
        assert!(validate_bits(operation, &[], &[0xf4]).is_some());
        for bytes in [&[][..], &[0x90], &[0xf4, 0xf4], &[0xf4, 0x90], &[0xee]] {
            assert!(validate_bits(operation, &[], bytes).is_none());
        }
        assert!(validate_bits(operation, &[0], &[0xf4]).is_none());
        let encoded = effects(operation);
        assert!(encoded.implicit_unit_clobbers.is_empty());
        let declaration = declaration(operation, crate::X86_64_PROCESSOR_HALT);
        assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
        assert_eq!(
            declaration.trap,
            MachineTrapBehavior::MayArchitecturalFaultV1
        );
        assert_eq!(
            declaration.alternatives[0].size,
            MachineSizeKnowledge::ExactBytes(1)
        );
    }

    #[test]
    fn every_operand_alias_preserves_port_and_model_specific_bits() {
        for operation in [
            Operation::PortRead,
            Operation::PortWrite,
            Operation::ModelSpecificRead,
            Operation::ModelSpecificWrite,
        ] {
            for first in 0..16 {
                for second in 0..16 {
                    if operation.produces_value() && second == 4 {
                        assert!(encode(operation, &[first, second]).is_err());
                        continue;
                    }
                    let bytes = encode(operation, &[first, second]).unwrap();
                    assert!(
                        validate_bits(operation, &[first, second], &bytes).is_some(),
                        "{operation:?} {first} {second}: {bytes:02x?}"
                    );
                    assert!(bytes.len() <= 40);
                    // Losing the physical opcode, part of a move, or the result
                    // assembly must never retain an apparently valid envelope.
                    for end in 0..bytes.len() {
                        assert!(
                            validate_bits(operation, &[first, second], &bytes[..end]).is_none(),
                            "accepted truncated {operation:?} at {end}"
                        );
                    }
                    let reversed = match operation {
                        Operation::PortRead => Operation::PortWrite,
                        Operation::PortWrite => Operation::PortRead,
                        Operation::ModelSpecificRead => Operation::ModelSpecificWrite,
                        Operation::ModelSpecificWrite => Operation::ModelSpecificRead,
                        _ => unreachable!(),
                    };
                    assert!(validate_bits(reversed, &[first, second], &bytes).is_none());
                }
            }
        }
    }

    #[test]
    fn decoder_rejects_lost_indices_values_and_result_halves() {
        let registers = [11, 10]; // The scratch-register parallel-copy cycle.
        for operation in [
            Operation::PortRead,
            Operation::PortWrite,
            Operation::ModelSpecificRead,
            Operation::ModelSpecificWrite,
        ] {
            let bytes = encode(operation, &registers).unwrap();
            // Mutate an actually observed source register in the first move.
            let mut changed = bytes.clone();
            changed[2] ^= 8;
            assert!(validate_bits(operation, &registers, &changed).is_none());
            let mut doubled = bytes.clone();
            doubled.extend(&bytes);
            assert!(validate_bits(operation, &registers, &doubled).is_none());
        }
        let mut read = encode(Operation::ModelSpecificRead, &[3, 12]).unwrap();
        let swap = read
            .windows(2)
            .position(|window| window == [0x0f, 0xca])
            .unwrap();
        read.drain(swap..swap + 2);
        assert!(validate_bits(Operation::ModelSpecificRead, &[3, 12], &read).is_none());
    }
}

#[cfg(test)]
mod encoding_vectors {
    use super::*;

    #[test]
    fn matches_independent_llvm_mc_vectors() {
        assert_eq!(
            encode(Operation::PortRead, &[3, 12]).unwrap(),
            [
                0x49, 0x89, 0xda, 0x44, 0x89, 0xd2, 0xec, 0x4c, 0x0f, 0xb6, 0xe0
            ]
        );
        assert_eq!(
            encode(Operation::ModelSpecificWrite, &[11, 10]).unwrap(),
            [
                0x4c, 0x89, 0xd8, 0x4d, 0x89, 0xd3, 0x49, 0x89, 0xc2, 0x44, 0x89, 0xd1, 0x4c, 0x89,
                0xda, 0x48, 0x0f, 0xca, 0x0f, 0xca, 0x44, 0x89, 0xd8, 0x0f, 0x30
            ]
        );
        assert_eq!(
            encode(Operation::ModelSpecificRead, &[3, 12]).unwrap(),
            [
                0x49, 0x89, 0xda, 0x44, 0x89, 0xd1, 0x0f, 0x32, 0x0f, 0xca, 0x48, 0x0f, 0xca, 0x4c,
                0x8d, 0x24, 0x10
            ]
        );
    }
}
