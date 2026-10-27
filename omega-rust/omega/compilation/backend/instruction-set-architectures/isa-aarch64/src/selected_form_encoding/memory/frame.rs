//! SP-relative frame stores and addresses, with independent instruction replay.
//! A `FrameAddress` is one `add xD, sp, #imm` inside the first 4 KiB of the
//! frame, a shifted-then-unshifted `add` pair through 16 MiB, and further
//! shifted page steps past it — the same steps the frame protocol moves `sp`
//! by — so every displacement a `u32` frame can resolve encodes exactly.
//! `Store64` always writes an allocated early-clobber address scratch with
//! the displacement's whole pages before its memory operation, which carries
//! the low twelve bits. The two-word form covers the first 16 MiB without
//! hidden register clobbers or an unused definition in a compact encoding.

use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint, MachineAlternativeFamily,
    MachineAlternativeKey, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    RegisterViewId, SelectedInstructionKind, ValidatedAarch64SelectedFormEncoding,
    ValidatedPhysicalRegisterModel, resolve_registers,
};
use crate::immediate_offset::{
    ADD_IMMEDIATE, address_words, append_page_steps, decode_address_words,
};

/// `xScratch = sp + whole pages`. A displacement inside the first page still
/// writes the scratch with `add xScratch, sp, #0, lsl #12`, so the declared
/// early-clobber write happens on every path.
fn store_address_words(scratch: u8, displacement: u32) -> Vec<u32> {
    let pages = u64::from(displacement >> 12);
    if pages == 0 {
        return vec![ADD_IMMEDIATE | (1 << 22) | (31 << 5) | u32::from(scratch)];
    }
    let mut words = Vec::new();
    append_page_steps(&mut words, ADD_IMMEDIATE, scratch, 31, pages);
    words
}

const fn store_word(register: u8, scratch: u8, displacement: u32) -> u32 {
    0xf900_0000 | (((displacement & 4095) / 8) << 10) | ((scratch as u32) << 5) | register as u32
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

pub fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    alignment_bytes: u16,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (register, scratch) = request(physical, kind, alternative, operands, displacement)?;
    let words = if alignment_bytes != 1 {
        let member = match kind {
            SelectedInstructionKind::FrameAddress { byte_offset, .. }
            | SelectedInstructionKind::Store64 { byte_offset, .. } => byte_offset,
            _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
        };
        if scratch.is_some() && !member.is_multiple_of(8) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
        let address = scratch.unwrap_or(register);
        let mut words = crate::immediate_offset::aligned_frame_address_words(
            address,
            displacement,
            alignment_bytes,
            member,
        )
        .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
        if scratch.is_some() {
            words.push(store_word(register, address, 0));
        }
        words
    } else if let Some(scratch) = scratch {
        // add xScratch, sp, #pages, lsl #12 per 4095 pages;
        // str xValue, [xScratch, #low].
        let mut words = store_address_words(scratch, displacement);
        words.push(store_word(register, scratch, displacement));
        words
    } else {
        address_words(register, 31, u64::from(displacement))
    };
    let bytes = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<u8>>();
    validate(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        alignment_bytes,
        &bytes,
    )
}

pub fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    alignment_bytes: u16,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let (register, scratch) = request(physical, kind, alternative, operands, displacement)?;
    let (chunks, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let words = chunks
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    if alignment_bytes != 1 {
        let member = match kind {
            SelectedInstructionKind::FrameAddress { byte_offset, .. }
            | SelectedInstructionKind::Store64 { byte_offset, .. } => byte_offset,
            _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
        };
        let address = scratch.unwrap_or(register);
        let prefix = if scratch.is_some() {
            if !member.is_multiple_of(8) || words.last() != Some(&store_word(register, address, 0))
            {
                return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
            }
            &words[..words.len() - 1]
        } else {
            words.as_slice()
        };
        if !crate::immediate_offset::validate_aligned_frame_address_words(
            prefix,
            address,
            displacement,
            alignment_bytes,
            member,
        ) {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
    } else if let Some(scratch) = scratch {
        let address = store_address_words(scratch, displacement);
        let [prefix @ .., store] = words.as_slice() else {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        };
        if prefix.len() != address.len() {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        // The page steps decode to the displacement's whole pages and the
        // store reads exactly the scratch they wrote.
        let pages = if displacement >> 12 == 0 {
            (prefix == address.as_slice()).then_some(0)
        } else {
            decode_address_words(prefix, scratch, 31)
        };
        if pages != Some(u64::from(displacement & !4095))
            || store & 0xffc0_0000 != 0xf900_0000
            || store & 31 != u32::from(register)
            || (store >> 5) & 31 != u32::from(scratch)
            || ((store >> 10) & 4095) * 8 != (displacement & 4095)
        {
            return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
        }
    } else {
        let expected = address_words(register, 31, u64::from(displacement));
        if words.len() != expected.len() {
            return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
        }
        if decode_address_words(&words, register, 31) != Some(u64::from(displacement)) {
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
    use crate::test_support::reject_every_bit_flip;
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn aligned_frame_forms_keep_sp_fixed_and_members_after_alignment() {
        let physical =
            register_model::validate_physical_register_model(aarch64_physical_register_model())
                .unwrap();
        let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
            operation: OperationId::new(104).unwrap(),
            place: PlaceId::new(102).unwrap(),
        });
        let value = physical.model().view_named("x9").unwrap().id;
        let scratch = physical.model().view_named("x10").unwrap().id;
        for alignment in [32_u16, 64, 4096, 32768] {
            for frame_offset in [0_u32, 16, 4096, 16_777_216] {
                for member in [0, 8, 24] {
                    let kind = SelectedInstructionKind::FrameAddress {
                        slot,
                        byte_offset: member,
                    };
                    let alternative = MachineAlternativeKey {
                        family: MachineAlternativeFamily::FrameAddress,
                        variant: 0,
                    };
                    let encoded = encode(
                        &physical,
                        kind,
                        alternative,
                        &[value],
                        frame_offset,
                        alignment,
                    )
                    .unwrap();
                    assert!(!encoded.footprint.writes_nzcv);
                    // Interpret the actual ADD/AND instruction fields under
                    // every possible ABI-aligned stack residue, not only a
                    // conveniently aligned test stack.
                    for residue in (0..u64::from(alignment)).step_by(16) {
                        let stack = 0x10_0000 + residue;
                        let mut address = stack;
                        for word in encoded
                            .bytes()
                            .chunks_exact(4)
                            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                        {
                            if word & 0xffc0_0000 == 0x9240_0000 {
                                let mask = (u64::MAX >> (63 - ((word >> 10) & 63)))
                                    .rotate_right((word >> 16) & 63);
                                address &= mask;
                            } else {
                                assert_eq!(
                                    word & 0xff80_0000,
                                    crate::immediate_offset::ADD_IMMEDIATE
                                );
                                let base = if (word >> 5) & 31 == 31 {
                                    stack
                                } else {
                                    address
                                };
                                let immediate = u64::from((word >> 10) & 4095)
                                    << if word & (1 << 22) != 0 { 12 } else { 0 };
                                address = base + immediate;
                            }
                        }
                        let home = address - u64::from(member);
                        assert!(home.is_multiple_of(u64::from(alignment)));
                        let reserved_start = stack + u64::from(frame_offset);
                        assert!(home >= reserved_start);
                        assert!(home - reserved_start <= u64::from(alignment - 16));
                    }
                    reject_every_bit_flip(encoded.bytes(), "frame form", |corrupt| {
                        validate(
                            &physical,
                            kind,
                            alternative,
                            &[value],
                            frame_offset,
                            alignment,
                            &corrupt,
                        )
                        .is_err()
                    });
                    let store = SelectedInstructionKind::Store64 {
                        slot,
                        byte_offset: member,
                    };
                    let store_alternative = MachineAlternativeKey {
                        family: MachineAlternativeFamily::Store64,
                        variant: 0,
                    };
                    let stored = encode(
                        &physical,
                        store,
                        store_alternative,
                        &[value, scratch],
                        frame_offset,
                        alignment,
                    )
                    .unwrap();
                    assert_eq!(stored.footprint.register_writes, vec![scratch]);
                    assert!(
                        validate(
                            &physical,
                            store,
                            store_alternative,
                            &[value, scratch],
                            frame_offset,
                            alignment,
                            stored.bytes()
                        )
                        .is_ok()
                    );
                }
            }
        }
    }

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
            // Past 16 MiB the address continues in further page steps,
            // through the last `u32` displacement.
            let displacements: &[u32] = if store {
                &[
                    0,
                    8,
                    32760,
                    32768,
                    35040,
                    65_536,
                    16_777_208,
                    16_777_216,
                    33_558_536,
                    u32::MAX - 7,
                ]
            } else {
                &[
                    0,
                    17,
                    4095,
                    4096,
                    4192,
                    16_777_215,
                    16_773_120,
                    33_554_431,
                    u32::MAX,
                ]
            };
            for displacement in displacements.iter().copied() {
                let mut operands = vec![physical.model().view_named("x9").unwrap().id];
                if store {
                    operands.push(physical.model().view_named("x10").unwrap().id);
                }
                let encoded =
                    encode(&physical, kind, alternative, &operands, displacement, 1).unwrap();
                assert_eq!(
                    encoded.footprint.encoded.implicit_unit_uses,
                    physical.model().view_named("sp").unwrap().units
                );
                if store {
                    let pages = u64::from(displacement >> 12).div_ceil(4095).max(1);
                    assert_eq!(encoded.bytes().len() as u64, (pages + 1) * 4);
                    assert_eq!(encoded.footprint.register_writes, vec![operands[1]]);
                    assert!(
                        encode(
                            &physical,
                            kind,
                            alternative,
                            &[operands[0], operands[0]],
                            displacement,
                            1
                        )
                        .is_err()
                    );
                } else {
                    assert_eq!(
                        encoded.bytes().len() as u64,
                        crate::immediate_offset::address_word_count(u64::from(displacement)) * 4
                    );
                }
                assert!(!encoded.footprint.writes_nzcv);
                reject_every_bit_flip(encoded.bytes(), "frame round trip", |corrupt| {
                    validate(
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                        1,
                        &corrupt,
                    )
                    .is_err()
                });
                assert!(
                    validate(
                        &physical,
                        kind,
                        alternative,
                        &[physical.model().view_named("x10").unwrap().id],
                        displacement,
                        1,
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
                        displacement ^ 8,
                        1,
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
                        displacement,
                        1
                    )
                    .is_err()
                );
                assert!(encode(&physical, kind, alternative, &[], displacement, 1).is_err());
            }
            if store {
                assert!(
                    encode(
                        &physical,
                        kind,
                        alternative,
                        &[physical.model().view_named("x9").unwrap().id],
                        1,
                        1
                    )
                    .is_err()
                );
            }
        }
    }
}
