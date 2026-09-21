//! Exact live-byte copy with explicit scratch registers and local control flow.

use super::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint,
};
use crate::selected_form_encoding::instruction_bytes::{modrm, rex};
use crate::selected_form_encoding::request_validation::resolve_registers;
pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [source, destination, count, cursor, byte] =
        request(physical, alternative, operands, displacement)?;
    let mut bytes = Vec::with_capacity(36);
    for register in [cursor, byte] {
        bytes.extend_from_slice(&[
            rex(register, 0, register),
            0x31,
            modrm(3, register, register),
        ]);
    }
    bytes.extend_from_slice(&[rex(count, 0, count), 0x85, modrm(3, count, count), 0x74, 25]);
    bytes.extend_from_slice(&[
        rex(byte, cursor, source),
        0x0f,
        0xb6,
        modrm(2, byte, 4),
        ((cursor & 7) << 3) | (source & 7),
        0,
        0,
        0,
        0,
    ]);
    bytes.extend_from_slice(&[
        rex(byte, cursor, destination) & !8,
        0x88,
        modrm(2, byte, 4),
        ((cursor & 7) << 3) | (destination & 7),
        0,
        0,
        0,
        0,
    ]);
    bytes.extend_from_slice(&[rex(0, 0, cursor), 0xff, modrm(3, 0, cursor)]);
    bytes.extend_from_slice(&[
        rex(count, 0, cursor),
        0x39,
        modrm(3, count, cursor),
        0x72,
        0xe7,
    ]);
    validate(physical, alternative, operands, displacement, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let [source, destination, count, cursor, byte] =
        request(physical, alternative, operands, displacement)?;
    // Decode roles and exact branch destinations, independently of construction.
    // The zero guard jumps past both accesses; the backedge targets the load.
    if bytes.len() != 36
        || !register_instruction(&bytes[0..3], 0x31, cursor, cursor)
        || !register_instruction(&bytes[3..6], 0x31, byte, byte)
        || !register_instruction(&bytes[6..9], 0x85, count, count)
        || bytes[9] != 0x74
        || 11_i16 + i16::from(bytes[10] as i8) != 36
        || !indexed_access(&bytes[11..20], true, source, cursor, byte)
        || !indexed_access(&bytes[20..28], false, destination, cursor, byte)
        || bytes[28] & 0xfe != 0x48
        || bytes[29] != 0xff
        || bytes[30] & 0xf8 != 0xc0
        || (bytes[30] & 7) | ((bytes[28] & 1) << 3) != cursor
        || !register_instruction(&bytes[31..34], 0x39, count, cursor)
        || bytes[34] != 0x72
        || 36_i16 + i16::from(bytes[35] as i8) != 11
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let flags = physical
        .model()
        .view_named("rflags")
        .ok_or(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel)?;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands[..3].to_vec(),
            register_writes: operands[3..].to_vec(),
            writes_rflags: true,
            encoded: MachineEncodedEffects {
                external_operand_reads: vec![0, 1, 2],
                external_operand_writes: vec![3, 4],
                implicit_unit_uses: Vec::new(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: flags.units.clone(),
                memory: MachineEncodedMemoryEffect::CopyBytesV1 {
                    source_pointer_operand: 0,
                    destination_pointer_operand: 1,
                    count_operand: 2,
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

fn register_instruction(bytes: &[u8], opcode: u8, register: u8, destination: u8) -> bool {
    let [prefix, operation, mode] = bytes else {
        return false;
    };
    prefix & 0xfa == 0x48
        && *operation == opcode
        && mode >> 6 == 3
        && ((mode >> 3) & 7) | ((prefix & 4) << 1) == register
        && (mode & 7) | ((prefix & 1) << 3) == destination
}

fn indexed_access(bytes: &[u8], load: bool, base: u8, cursor: u8, byte: u8) -> bool {
    let (prefix, mode, sib, displacement) = if load {
        let [prefix, 0x0f, 0xb6, mode, sib, displacement @ ..] = bytes else {
            return false;
        };
        (*prefix, *mode, *sib, displacement)
    } else {
        let [prefix, 0x88, mode, sib, displacement @ ..] = bytes else {
            return false;
        };
        (*prefix, *mode, *sib, displacement)
    };
    prefix & 0xf8 == if load { 0x48 } else { 0x40 }
        && mode & 0xc7 == 0x84
        && sib >> 6 == 0
        && displacement == [0; 4]
        && ((mode >> 3) & 7) | ((prefix & 4) << 1) == byte
        && (sib & 7) | ((prefix & 1) << 3) == base
        && ((sib >> 3) & 7) | ((prefix & 2) << 2) == cursor
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<[u8; 5], X86_64SelectedFormEncodingError> {
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    if displacement != 0
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::CopyBytes,
                variant: 0,
            })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers: [u8; 5] = resolve_registers(physical, operands)?
        .try_into()
        .map_err(|_| X86_64SelectedFormEncodingError::OperandCountMismatch)?;
    if registers[..3].contains(&registers[3]) || registers[..4].contains(&registers[4]) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(registers)
}

#[cfg(test)]
mod tests {
    use super::super::{
        SelectedInstructionKind, encode_x86_64_selected_form,
        validate_x86_64_selected_form_encoding,
    };
    use super::{MachineAlternativeFamily, MachineAlternativeKey, encode, validate};

    #[test]
    fn copy_bytes_catalog_requires_both_early_clobbers_and_exact_dynamic_effect() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let catalog = crate::x86_64_register_constraint_catalog(&physical);
        let constraints =
            crate::validate_x86_64_register_constraint_catalog(catalog.clone(), &physical).unwrap();
        for operand in [3, 4] {
            let mut changed = catalog.clone();
            changed
                .constraints
                .iter_mut()
                .find(|row| row.key == crate::X86_64_COPY_BYTES)
                .unwrap()
                .operands[operand]
                .early_clobber = false;
            assert!(
                crate::validate_x86_64_register_constraint_catalog(changed, &physical).is_err()
            );
        }
        for target in [
            target::NativeTarget::linux_x64(),
            target::NativeTarget::windows_x64(),
        ] {
            let effects = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, effects.clone())
                .unwrap();
            let row = effects
                .declarations
                .iter()
                .find(|row| row.semantic == selected_instructions::MachineSemanticKind::CopyBytes)
                .unwrap();
            assert_eq!(
                row.memory,
                selected_instructions::MachineMemoryEffect::CopyBytesV1
            );
            assert_eq!(
                row.alternatives[0].encoded.external_operand_reads,
                [0, 1, 2]
            );
            assert_eq!(row.alternatives[0].encoded.external_operand_writes, [3, 4]);
        }
    }

    #[test]
    fn copy_bytes_preserves_inputs_and_rejects_every_changed_encoding_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::CopyBytes,
            variant: 0,
        };
        for names in [
            ["r13", "r14", "r15", "r12", "rbp"],
            ["rax", "rcx", "rdx", "rsi", "rdi"],
        ] {
            let operands = names.map(|name| physical.model().view_named(name).unwrap().id);
            let encoded = encode(&physical, alternative, &operands, 0).unwrap();
            assert_eq!(
                encode_x86_64_selected_form(
                    &physical,
                    SelectedInstructionKind::CopyBytes,
                    alternative,
                    &operands
                )
                .unwrap(),
                encoded
            );
            validate_x86_64_selected_form_encoding(
                &physical,
                SelectedInstructionKind::CopyBytes,
                alternative,
                &operands,
                encoded.bytes(),
            )
            .unwrap();
            assert_eq!(encoded.bytes().len(), 36);
            assert_eq!(encoded.footprint().register_reads, operands[..3]);
            assert_eq!(encoded.footprint().register_writes, operands[3..]);
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate(&physical, alternative, &operands, 0, &changed).is_err(),
                    "bit {bit}"
                );
            }
            for length in 0..encoded.bytes().len() {
                assert!(
                    validate(
                        &physical,
                        alternative,
                        &operands,
                        0,
                        &encoded.bytes()[..length]
                    )
                    .is_err()
                );
            }
            let mut trailing = encoded.bytes().to_vec();
            trailing.push(0);
            assert!(validate(&physical, alternative, &operands, 0, &trailing).is_err());
            assert!(encode(&physical, alternative, &operands, 1).is_err());
            for operand in 0..5 {
                let mut changed = operands;
                changed[operand] = physical.model().view_named("rsp").unwrap().id;
                assert!(encode(&physical, alternative, &changed, 0).is_err());
            }
            for scratch in 3..5 {
                for other in 0..5 {
                    if other == scratch {
                        continue;
                    }
                    let mut changed = operands;
                    changed[scratch] = operands[other];
                    assert!(encode(&physical, alternative, &changed, 0).is_err());
                }
            }
        }
    }
}
