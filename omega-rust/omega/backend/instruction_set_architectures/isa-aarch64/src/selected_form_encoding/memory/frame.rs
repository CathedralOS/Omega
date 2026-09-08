//! SP-relative frame stores and addresses, with independent instruction replay.
//! Unshifted ADD covers the entire existing frame protocol's <=4095-byte range,
//! including an empty home's one-past address; larger frames remain rejected.
use super::*;

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<(u8, bool), Aarch64SelectedFormEncodingError> {
    let store = matches!(kind, SelectedInstructionKind::Store64 { .. });
    let family = if store {
        MachineAlternativeFamily::Store64
    } else {
        MachineAlternativeFamily::FrameAddress
    };
    if physical.model() != &aarch64_physical_register_model()
        || !matches!(
            kind,
            SelectedInstructionKind::Store64 { .. } | SelectedInstructionKind::FrameAddress { .. }
        )
        || alternative != (MachineAlternativeKey { family, variant: 0 })
        || if store {
            !displacement.is_multiple_of(8) || displacement > 32760
        } else {
            displacement > 4095
        }
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let [register] = registers.as_slice() else {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    };
    Ok((*register, store))
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (register, store) = request(physical, kind, alternative, operands, displacement)?;
    let (opcode, immediate) = if store {
        (0xf900_0000, displacement / 8)
    } else {
        (0x9100_0000, displacement)
    };
    let word = opcode | (immediate << 10) | (31 << 5) | u32::from(register);
    validate(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &word.to_le_bytes(),
    )
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (register, store) = request(physical, kind, alternative, operands, displacement)?;
    let word = bytes
        .try_into()
        .ok()
        .map(u32::from_le_bytes)
        .ok_or(Aarch64SelectedFormEncodingError::MalformedEncoding)?;
    let opcode = if store { 0xf900_0000 } else { 0x9100_0000 };
    let scale = if store { 8 } else { 1 };
    if word & 0xffc0_0000 != opcode
        || word & 31 != u32::from(register)
        || (word >> 5) & 31 != 31
        || ((word >> 10) & 4095) * scale != displacement
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let stack_pointer = physical
        .model()
        .view_named("sp")
        .expect("canonical stack pointer");
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: if store { operands.to_vec() } else { Vec::new() },
            register_writes: if store { Vec::new() } else { operands.to_vec() },
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: if store { vec![0] } else { Vec::new() },
                external_operand_writes: if store { Vec::new() } else { vec![0] },
                implicit_unit_uses: stack_pointer.units.clone(),
                implicit_unit_defs: Vec::new(),
                implicit_unit_clobbers: Vec::new(),
                memory: if store {
                    MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                        stack_pointer: stack_pointer.id,
                        byte_count: 8,
                    }
                } else {
                    MachineEncodedMemoryEffect::NoneV1
                },
                stack: MachineEncodedStackEffect::UnchangedV1,
                trap: if store {
                    MachineEncodedTrapBehavior::MayArchitecturalFaultV1
                } else {
                    MachineEncodedTrapBehavior::NeverV1
                },
                control: MachineEncodedControlEffect::FallThroughV1,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn local_frame_forms_replay_sp_offset_register_and_all_opcode_bits() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let slot = FrameStorageSlotId::Local(LocalStorageSlotId {
            operation: OperationId::new(104).unwrap(),
            place: PlaceId::new(102).unwrap(),
        });
        for store in [false, true] {
            let kind = if store {
                SelectedInstructionKind::Store64 {
                    slot,
                    byte_offset: 0,
                }
            } else {
                SelectedInstructionKind::FrameAddress {
                    slot,
                    byte_offset: 0,
                }
            };
            let alternative = MachineAlternativeKey {
                family: if store {
                    MachineAlternativeFamily::Store64
                } else {
                    MachineAlternativeFamily::FrameAddress
                },
                variant: 0,
            };
            for displacement in if store { [0, 8, 32760] } else { [0, 17, 4095] } {
                let operands = [physical.model().view_named("x9").unwrap().id];
                let encoded =
                    encode(&physical, kind, alternative, &operands, displacement).unwrap();
                assert_eq!(
                    encoded.footprint.encoded.implicit_unit_uses,
                    physical.model().view_named("sp").unwrap().units
                );
                assert!(!encoded.footprint.writes_nzcv);
                for bit in 0..32 {
                    let mut corrupt = encoded.bytes().to_vec();
                    corrupt[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate(
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &corrupt
                        )
                        .is_err()
                    );
                }
                assert!(
                    validate(
                        &physical,
                        kind,
                        alternative,
                        &[physical.model().view_named("x10").unwrap().id],
                        displacement,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    validate(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement + 8,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    encode(
                        &physical,
                        kind,
                        alternative,
                        &[physical.model().view_named("sp").unwrap().id],
                        displacement
                    )
                    .is_err()
                );
                assert!(
                    encode(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        if store { 32768 } else { 4096 }
                    )
                    .is_err()
                );
                assert!(encode(&physical, kind, alternative, &[], displacement).is_err());
            }
            if store {
                assert!(
                    encode(
                        &physical,
                        kind,
                        alternative,
                        &[physical.model().view_named("x9").unwrap().id],
                        1
                    )
                    .is_err()
                );
            }
        }
    }
}
