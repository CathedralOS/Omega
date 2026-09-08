//! Concrete Linux write-one-byte leaf: caller frame storage, no hidden stack adjustment.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;

pub(crate) fn effects() -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical Linux write register")
    };
    let mut uses = view("sp").units.clone();
    uses.extend(view("pc").units.iter().copied());
    uses.sort_unstable();
    uses.dedup();
    let mut clobbers = Vec::new();
    for name in ["x0", "x1", "x2", "x8", "nzcv"] {
        clobbers.extend(view(name).units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::LinuxWriteByteV1 {
            stack_pointer: view("sp").id,
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::LinuxWriteFailureV1,
        control: MachineEncodedControlEffect::LinuxWriteReturnOrTrapV1,
    }
}

pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::LinuxWriteByteI32,
        constraint,
        memory: MachineMemoryEffect::LinuxWriteByteV1,
        trap: MachineTrapBehavior::LinuxWriteFailureV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::LinuxWriteByteI32,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(36),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(),
        }],
    }
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if physical.model() != &crate::aarch64_physical_register_model()
        || !matches!(
            kind,
            SelectedInstructionKind::LinuxWriteByteI32 {
                slot: LocalStorageSlotId::Boundary { .. }
            }
        )
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::LinuxWriteByteI32,
                variant: 0,
            })
        || displacement > 4095
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let [register] = registers.as_slice() else {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    };
    Ok(*register)
}

pub fn encode_aarch64_selected_linux_write_byte_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = request(physical, kind, alternative, operands, displacement)?;
    let words = [
        0x3900_03e0 | (displacement << 10) | u32::from(register),
        0xd280_0020,
        0x9100_03e1 | (displacement << 10),
        0xd280_0022,
        0xd280_0808,
        0xd400_0001,
        0xf100_001f,
        0x5400_004c,
        0xd420_0000,
    ];
    let bytes = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    validate_aarch64_selected_linux_write_byte_form(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

pub fn validate_aarch64_selected_linux_write_byte_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = request(physical, kind, alternative, operands, displacement)?;
    if decode(bytes) != Some((register, displacement)) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands.to_vec(),
            register_writes: Vec::new(),
            writes_nzcv: true,
            encoded: effects(),
        },
    })
}

fn decode(bytes: &[u8]) -> Option<(u8, u32)> {
    if bytes.len() != 36 {
        return None;
    }
    let words = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    if words[0] & 0xffc0_03e0 != 0x3900_03e0
        || words[0] & 31 == 31
        || words[2] & 0xffc0_03ff != 0x9100_03e1
        || ((words[2] >> 10) & 4095) != ((words[0] >> 10) & 4095)
        || words[1] != 0xd280_0020
        || words[3..]
            != [
                0xd280_0022,
                0xd280_0808,
                0xd400_0001,
                0xf100_001f,
                0x5400_004c,
                0xd420_0000,
            ]
    {
        return None;
    }
    Some(((words[0] & 31) as u8, (words[0] >> 10) & 4095))
}

/// Independently decode the complete returning write leaf for object custody replay.
pub fn decode_aarch64_selected_linux_write_byte_i32(
    bytes: &[u8],
) -> Option<(calling_conventions::MachineRegister, u32)> {
    let (register, displacement) = decode(bytes)?;
    Some((
        calling_conventions::MachineRegister::Aarch64X(register),
        displacement,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn linux_write_byte_replays_source_frame_offset_syscall_failure_branch_and_every_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let kind = SelectedInstructionKind::LinuxWriteByteI32 {
            slot: LocalStorageSlotId::Boundary {
                operation: OperationId::new(7).unwrap(),
            },
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::LinuxWriteByteI32,
            variant: 0,
        };
        for name in ["x0", "x1", "x2", "x8", "x9", "x28"] {
            let operands = [physical.model().view_named(name).unwrap().id];
            for displacement in [0, 31, 4095] {
                let encoded = encode_aarch64_selected_linux_write_byte_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.footprint().encoded, effects());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
                for bit in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate_aarch64_selected_linux_write_byte_form(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &changed
                        )
                        .is_err(),
                        "encoded bit {bit}"
                    );
                }
                assert!(
                    validate_aarch64_selected_linux_write_byte_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement ^ 1,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    validate_aarch64_selected_linux_write_byte_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                        &encoded.bytes()[1..]
                    )
                    .is_err()
                );
                let mut trailing = encoded.bytes().to_vec();
                trailing.push(0);
                assert!(
                    validate_aarch64_selected_linux_write_byte_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                        &trailing
                    )
                    .is_err()
                );
                let wrong_slot = SelectedInstructionKind::LinuxWriteByteI32 {
                    slot: LocalStorageSlotId::Structural {
                        operation: OperationId::new(7).unwrap(),
                        place: PlaceId::new(1).unwrap(),
                    },
                };
                assert!(
                    validate_aarch64_selected_linux_write_byte_form(
                        &physical,
                        wrong_slot,
                        alternative,
                        &operands,
                        displacement,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    encode_aarch64_selected_linux_write_byte_form(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        4096
                    )
                    .is_err()
                );
                assert!(
                    encode_aarch64_selected_linux_write_byte_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement
                    )
                    .is_err()
                );
            }
        }
    }
}
