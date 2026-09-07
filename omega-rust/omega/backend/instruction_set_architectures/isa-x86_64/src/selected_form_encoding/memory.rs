//! AMD64 ordinary load/store/address primitives with independent byte replay.
use super::*;

pub fn encode_x86_64_selected_memory_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (opcode, register, base, _) = request(physical, kind, alternative, operands)?;
    let displacement = i32::try_from(displacement)
        .map_err(|_| X86_64SelectedFormEncodingError::ImmediateOutsideU12)?;
    let mut bytes = vec![rex(register, 0, base), opcode, modrm(2, register, base)];
    if base & 7 == 4 {
        bytes.push(0x24);
    }
    bytes.extend_from_slice(&displacement.to_le_bytes());
    validate_x86_64_selected_memory_form(
        physical,
        kind,
        alternative,
        operands,
        displacement as u32,
        &bytes,
    )
}

pub fn validate_x86_64_selected_memory_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (opcode, register, base, footprint) = request(physical, kind, alternative, operands)?;
    let prefix = *bytes
        .first()
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let mode = *bytes
        .get(2)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let has_sib = mode & 7 == 4;
    let displacement_start = if has_sib { 4 } else { 3 };
    let actual = bytes
        .get(displacement_start..)
        .filter(|tail| tail.len() == 4)
        .and_then(|tail| <[u8; 4]>::try_from(tail).ok())
        .map(i32::from_le_bytes)
        .ok_or(X86_64SelectedFormEncodingError::MalformedEncoding)?;
    let decoded_register = ((mode >> 3) & 7) | ((prefix & 4) << 1);
    let decoded_base = (mode & 7) | ((prefix & 1) << 3);
    if prefix & 0xfa != 0x48
        || bytes[1] != opcode
        || mode >> 6 != 2
        || decoded_register != register
        || decoded_base != base
        || (has_sib && bytes.get(3) != Some(&0x24))
        || actual < 0
        || actual as u32 != displacement
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint,
    })
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(u8, u8, u8, X86_64SelectedFormFootprint), X86_64SelectedFormEncodingError> {
    if physical.model() != &crate::x86_64_physical_register_model() {
        return Err(X86_64SelectedFormEncodingError::NonCanonicalPhysicalModel);
    }
    let (family, count, opcode, key) = match kind {
        SelectedInstructionKind::Load64 { .. } => (
            MachineAlternativeFamily::Load64,
            2,
            0x8b,
            crate::X86_64_LOAD64,
        ),
        SelectedInstructionKind::Store64 { .. } => (
            MachineAlternativeFamily::Store64,
            1,
            0x89,
            crate::X86_64_STORE64,
        ),
        SelectedInstructionKind::FrameAddress { .. } => (
            MachineAlternativeFamily::FrameAddress,
            1,
            0x8d,
            crate::X86_64_FRAME_ADDRESS,
        ),
        _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
    };
    if alternative != (MachineAlternativeKey { family, variant: 0 }) {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    if operands.len() != count {
        return Err(X86_64SelectedFormEncodingError::OperandCountMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let (register, base) = if count == 2 {
        (registers[1], registers[0])
    } else {
        (registers[0], 4)
    };
    let catalog = crate::x86_64_register_constraint_catalog(physical);
    let constraint = catalog
        .constraints
        .iter()
        .find(|row| row.key == key)
        .expect("canonical memory constraint");
    let stack_pointer = physical
        .model()
        .view_named("rsp")
        .expect("canonical rsp")
        .id;
    let (reads, writes, memory, trap) = match kind {
        SelectedInstructionKind::Load64 { .. } => (
            vec![0],
            vec![1],
            MachineEncodedMemoryEffect::ReadPointerV1 {
                pointer_operand: 0,
                byte_count: 8,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ),
        SelectedInstructionKind::Store64 { .. } => (
            vec![0],
            vec![],
            MachineEncodedMemoryEffect::WriteOutgoingArgumentV1 {
                stack_pointer,
                byte_count: 8,
            },
            MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        ),
        _ => (
            vec![],
            vec![0],
            MachineEncodedMemoryEffect::NoneV1,
            MachineEncodedTrapBehavior::NeverV1,
        ),
    };
    let footprint = X86_64SelectedFormFootprint {
        register_reads: reads
            .iter()
            .map(|index| operands[*index as usize])
            .collect(),
        register_writes: writes
            .iter()
            .map(|index| operands[*index as usize])
            .collect(),
        writes_rflags: false,
        encoded: MachineEncodedEffects {
            external_operand_reads: reads,
            external_operand_writes: writes,
            implicit_unit_uses: constraint.implicit_uses.clone(),
            implicit_unit_defs: constraint.implicit_defs.clone(),
            implicit_unit_clobbers: constraint.clobbers.clone(),
            memory,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap,
            control: MachineEncodedControlEffect::FallThroughV1,
        },
    };
    Ok((opcode, register, base, footprint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_model::validate_physical_register_model;
    use selected_instructions::OutgoingArgumentSlotId;
    use semantic_vocabulary::OperationId;

    #[test]
    fn stack_pointer_is_not_an_allocatable_pointer_operand() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let stack_pointer = physical.model().view_named("rsp").unwrap().id;
        let result = physical.model().view_named("r11").unwrap().id;
        assert_eq!(
            encode_x86_64_selected_memory_form(
                &physical,
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Load64,
                    variant: 0
                },
                &[stack_pointer, result],
                0,
            ),
            Err(X86_64SelectedFormEncodingError::UnknownOrNonGpr64View(
                stack_pointer
            )),
        );
    }

    #[test]
    fn ordinary_memory_forms_replay_registers_displacements_and_every_encoded_byte() {
        let physical =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let slot = OutgoingArgumentSlotId {
            operation: OperationId::new(7).unwrap(),
            argument_index: 1,
        };
        for name in ["rax", "rbp", "r8", "r12", "r15"] {
            let pointer = physical.model().view_named(name).unwrap().id;
            let value = physical.model().view_named("r11").unwrap().id;
            for (kind, family, operands) in [
                (
                    SelectedInstructionKind::Load64 { byte_offset: 8 },
                    MachineAlternativeFamily::Load64,
                    vec![pointer, value],
                ),
                (
                    SelectedInstructionKind::Store64 {
                        slot,
                        byte_offset: 8,
                    },
                    MachineAlternativeFamily::Store64,
                    vec![value],
                ),
                (
                    SelectedInstructionKind::FrameAddress {
                        slot,
                        byte_offset: 0,
                    },
                    MachineAlternativeFamily::FrameAddress,
                    vec![value],
                ),
            ] {
                let alternative = MachineAlternativeKey { family, variant: 0 };
                let encoded =
                    encode_x86_64_selected_memory_form(&physical, kind, alternative, &operands, 56)
                        .unwrap();
                if name == "r12" {
                    let expected = match kind {
                        SelectedInstructionKind::Load64 { .. } => {
                            vec![0x4d, 0x8b, 0x9c, 0x24, 56, 0, 0, 0]
                        }
                        SelectedInstructionKind::Store64 { .. } => {
                            vec![0x4c, 0x89, 0x9c, 0x24, 56, 0, 0, 0]
                        }
                        SelectedInstructionKind::FrameAddress { .. } => {
                            vec![0x4c, 0x8d, 0x9c, 0x24, 56, 0, 0, 0]
                        }
                        _ => unreachable!(),
                    };
                    assert_eq!(encoded.bytes, expected);
                }
                assert_eq!(
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        56,
                        &encoded.bytes
                    ),
                    Ok(encoded.clone())
                );
                for byte_index in 0..encoded.bytes.len() {
                    let mut changed = encoded.bytes.clone();
                    changed[byte_index] ^= 1;
                    assert!(
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            56,
                            &changed
                        )
                        .is_err(),
                        "{name} {kind:?} byte {byte_index}"
                    );
                }
                assert!(
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        48,
                        &encoded.bytes
                    )
                    .is_err()
                );
                for end in 0..encoded.bytes.len() {
                    assert!(
                        validate_x86_64_selected_memory_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            56,
                            &encoded.bytes[..end]
                        )
                        .is_err()
                    );
                }
                let mut trailing = encoded.bytes.clone();
                trailing.push(0);
                assert!(
                    validate_x86_64_selected_memory_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        56,
                        &trailing
                    )
                    .is_err()
                );
            }
        }
    }
}
