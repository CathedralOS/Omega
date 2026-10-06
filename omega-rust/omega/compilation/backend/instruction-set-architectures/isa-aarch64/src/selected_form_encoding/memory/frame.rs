//! SP-relative frame stores and addresses, with independent instruction replay.
//! A `FrameAddress` is one `add xD, sp, #imm` inside the first 4 KiB of the
//! frame and a shifted-then-unshifted `add` pair past it — the same immediate
//! reach the frame protocol's `sp` adjustment admits — so every displacement
//! a committable frame can resolve encodes exactly. `Store64` always writes
//! an allocated early-clobber address scratch before its memory operation.
//! The two-word form covers the same frame extent without hidden register
//! clobbers or an unused definition in a compact encoding.

use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    RegisterViewId, SelectedInstructionKind, ValidatedAarch64SelectedFormEncoding,
    ValidatedPhysicalRegisterModel, resolve_registers,
};
const fn frame_address_words(register: u8, displacement: u32) -> (u32, Option<u32>) {
    let high = displacement >> 12;
    if high == 0 {
        (
            0x9100_0000 | (displacement << 10) | (31 << 5) | register as u32,
            None,
        )
    } else {
        let low = displacement & 4095;
        (
            0x9140_0000 | (high << 10) | (31 << 5) | register as u32,
            if low == 0 {
                None
            } else {
                Some(0x9100_0000 | (low << 10) | ((register as u32) << 5) | register as u32)
            },
        )
    }
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<(u8, Option<u8>), Aarch64SelectedFormEncodingError> {
    let store = matches!(kind, SelectedInstructionKind::Store64 { .. });
    let family = if store {
        MachineAlternativeFamily::Store64
    } else {
        MachineAlternativeFamily::FrameAddress
    };
    if matches!(
        kind,
        SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Incoming { .. },
            ..
        }
    ) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || !matches!(
            kind,
            SelectedInstructionKind::Store64 { .. } | SelectedInstructionKind::FrameAddress { .. }
        )
        || alternative != (MachineAlternativeKey { family, variant: 0 })
        || (store && !displacement.is_multiple_of(8))
        || u64::from(displacement) > crate::frame_protocol::MAX_STACK_POINTER_ADJUST_BYTES
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    match registers.as_slice() {
        [value, scratch] if store && value != scratch => Ok((*value, Some(*scratch))),
        [register] if !store => Ok((*register, None)),
        _ => Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
    }
}

pub(super) fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (register, scratch) = request(physical, kind, alternative, operands, displacement)?;
    let bytes: Vec<u8> = if let Some(scratch) = scratch {
        // add xScratch, sp, #high, lsl #12; str xValue, [xScratch, #low].
        let address = 0x9140_03e0 | ((displacement >> 12) << 10) | u32::from(scratch);
        let store = 0xf900_0000
            | (((displacement & 4095) / 8) << 10)
            | (u32::from(scratch) << 5)
            | u32::from(register);
        [address, store]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    } else {
        let (first, second) = frame_address_words(register, displacement);
        first
            .to_le_bytes()
            .into_iter()
            .chain(second.into_iter().flat_map(u32::to_le_bytes))
            .collect()
    };
    validate(physical, kind, alternative, operands, displacement, &bytes)
}

pub(super) fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (register, scratch) = request(physical, kind, alternative, operands, displacement)?;
    if let Some(scratch) = scratch {
        let words: [u8; 8] = bytes
            .try_into()
            .map_err(|_| Aarch64SelectedFormEncodingError::MalformedEncoding)?;
        let address = u32::from_le_bytes([words[0], words[1], words[2], words[3]]);
        let store = u32::from_le_bytes([words[4], words[5], words[6], words[7]]);
        if address & 0xffc0_03e0 != 0x9140_03e0
            || address & 31 != u32::from(scratch)
            || (address >> 10) & 4095 != displacement >> 12
            || store & 0xffc0_0000 != 0xf900_0000
            || store & 31 != u32::from(register)
            || (store >> 5) & 31 != u32::from(scratch)
            || ((store >> 10) & 4095) * 8 != (displacement & 4095)
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
    } else {
        let (first, second) = frame_address_words(register, displacement);
        let expected = first
            .to_le_bytes()
            .into_iter()
            .chain(second.into_iter().flat_map(u32::to_le_bytes))
            .collect::<Vec<u8>>();
        if bytes.len() != expected.len() {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        if bytes != expected {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
    }
    let store = scratch.is_some();
    let stack_pointer = physical
        .model()
        .view_named("sp")
        .expect("canonical stack pointer");
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: if store { vec![operands[0]] } else { Vec::new() },
            register_writes: vec![operands[usize::from(store)]],
            writes_nzcv: false,
            encoded: MachineEncodedEffects {
                external_operand_reads: if store { vec![0] } else { Vec::new() },
                external_operand_writes: vec![u16::from(store)],
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
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind, encode, validate,
    };
    use crate::aarch64_physical_register_model;
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn local_frame_forms_replay_sp_offset_register_and_all_opcode_bits() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
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
            let displacements: &[u32] = if store {
                &[0, 8, 32760, 32768, 35040, 65_536, 16_777_208]
            } else {
                &[0, 17, 4095, 4096, 4192, 16_777_215]
            };
            for displacement in displacements.iter().copied() {
                let mut operands = vec![physical.model().view_named("x9").unwrap().id];
                if store {
                    operands.push(physical.model().view_named("x10").unwrap().id);
                }
                let encoded =
                    encode(&physical, kind, alternative, &operands, displacement).unwrap();
                assert_eq!(
                    encoded.footprint.encoded.implicit_unit_uses,
                    physical.model().view_named("sp").unwrap().units
                );
                if store {
                    assert_eq!(encoded.bytes().len(), 8);
                    assert_eq!(encoded.footprint.register_writes, vec![operands[1]]);
                    assert!(
                        encode(
                            &physical,
                            kind,
                            alternative,
                            &[operands[0], operands[0]],
                            displacement
                        )
                        .is_err()
                    );
                } else {
                    assert_eq!(
                        encoded.bytes().len(),
                        if displacement <= 4095 || displacement % 4096 == 0 {
                            4
                        } else {
                            8
                        }
                    );
                }
                assert!(!encoded.footprint.writes_nzcv);
                for bit in 0..(encoded.bytes().len() * 8) {
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
                assert!(encode(&physical, kind, alternative, &operands, 16_777_216).is_err());
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
