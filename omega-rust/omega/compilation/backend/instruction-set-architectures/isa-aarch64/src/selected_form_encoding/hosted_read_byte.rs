//! Hosted read-one-byte leaf: caller frame storage, no hidden stack adjustment.
use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use register_model::RegisterConstraintKey;
use register_model::RegisterViewId;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    LocalStorageSlotId, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
};
use target::NativeTarget;

/// Fixed words around the payload-address steps; Darwin forms the address
/// twice because its system call return writes `x1`.
fn size(target: NativeTarget) -> MachineSizeKnowledge {
    let address = crate::immediate_offset::MAX_ADDRESS_BYTES;
    if target == NativeTarget::macos_arm64() {
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 64,
            maximum_bytes: Some(56 + 2 * address),
        }
    } else {
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 56,
            maximum_bytes: Some(52 + address),
        }
    }
}

pub(crate) fn effects(target: NativeTarget) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical hosted read register")
    };
    let mut uses = view("sp").units.clone();
    uses.extend(view("pc").units.iter().copied());
    uses.sort_unstable();
    uses.dedup();
    let mut clobbers = Vec::new();
    let syscall_register = if target == NativeTarget::macos_arm64() {
        "x16"
    } else {
        "x8"
    };
    for name in ["x0", "x1", "x2", syscall_register, "x9", "nzcv"] {
        clobbers.extend(view(name).units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::HostedReadByteV1 {
            stack_pointer: view("sp").id,
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::HostedReadFailureV1,
        control: MachineEncodedControlEffect::HostedReadReturnOrTrapV1,
    }
}

pub(crate) fn declaration(
    target: NativeTarget,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::HostedReadByte,
        constraint,
        memory: MachineMemoryEffect::HostedReadByteV1,
        trap: MachineTrapBehavior::HostedReadFailureV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedReadByte,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: size(target),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(target),
        }],
    }
}

fn request(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<(), Aarch64SelectedFormEncodingError> {
    // Hosted structural storage is committed caller frame space: its
    // displacement is a nonnegative word offset whose payload lies within a
    // `u32` frame.
    if (target != NativeTarget::linux_arm64() && target != NativeTarget::macos_arm64())
        || physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || !matches!(
            kind,
            SelectedInstructionKind::HostedReadByte {
                slot: LocalStorageSlotId::Structural { .. }
            }
        )
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedReadByte,
                variant: 0,
            })
        || !operands.is_empty()
        || displacement < 0
        || displacement % 4 != 0
        || displacement
            .checked_add(4)
            .is_none_or(|payload| u32::try_from(payload).is_err())
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

/// Encode a hosted read into an eight-byte caller-owned structural home.
pub fn encode_aarch64_selected_hosted_read_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    request(target, physical, kind, alternative, operands, displacement)?;
    let displacement = u32::try_from(displacement)
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    let bytes = if target == NativeTarget::macos_arm64() {
        crate::encode_macos_read_byte_to_stack(displacement, displacement + 4)
    } else {
        crate::encode_linux_read_byte_to_stack(displacement, displacement + 4)
    }
    .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    validate_aarch64_selected_hosted_read_byte_form(
        target,
        physical,
        kind,
        alternative,
        operands,
        i64::from(displacement),
        &bytes,
    )
}

/// Replay every encoded instruction and address independently of the encoder.
pub fn validate_aarch64_selected_hosted_read_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    request(target, physical, kind, alternative, operands, displacement)?;
    if decode_aarch64_selected_hosted_read_byte(target, bytes).map(i64::from) != Some(displacement)
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            writes_nzcv: true,
            encoded: effects(target),
        },
    })
}

/// Decode the complete target-specific read leaf, returning its structural home byte offset.
pub fn decode_aarch64_selected_hosted_read_byte(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
    let (word_bytes, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return None;
    }
    let words = word_bytes
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    // The payload address in `x1`: one unshifted `add x1, sp, #imm`, or
    // shifted page steps followed by their nonzero unshifted remainder. The
    // prefix ends at the first word that is not an `add` into `x1`; the tag
    // store that follows never is.
    let reach = words
        .iter()
        .take_while(|word| *word & 0xff80_001f == 0x9100_0001)
        .count();
    let address = &words[..reach];
    let payload = u32::try_from(crate::immediate_offset::decode_address_words(
        address, 1, 31,
    )?)
    .ok()?;
    if !payload.is_multiple_of(4)
        || payload < 4
        || crate::hosted_sequences::read_byte_payload_address(payload).as_slice() != address
    {
        return None;
    }
    let reach = u32::try_from(reach).ok()?;
    // Linux system calls preserve `x1`; Darwin's return writes a second
    // result there, so its sequence forms the address again before the tag.
    let (calls, readdress): (&[u32], bool) = if target == NativeTarget::linux_arm64() {
        (
            &[
                0xd280_0000,
                0xd280_0022,
                0xd280_07e8,
                0xd400_0001,
                0xb400_00e0,
                0xf100_041f,
                0x5400_0081,
                0x5280_0029,
            ],
            false,
        )
    } else if target == NativeTarget::macos_arm64() {
        (
            &[
                0xd280_0000,
                0xd280_0022,
                0xd280_0070,
                0xd400_1001,
                0x5400_0002 | ((7 + reach) << 5),
                0xb400_0000 | ((7 + reach) << 5),
                0xf100_041f,
                0x5400_0001 | ((4 + reach) << 5),
                0x5280_0029,
            ],
            true,
        )
    } else {
        return None;
    };
    let mut expected = address.to_vec();
    expected.extend([0xb81f_c03f, 0xb900_003f]);
    expected.extend_from_slice(calls);
    if readdress {
        expected.extend_from_slice(address);
    }
    expected.extend([0xb81f_c029, 0x1400_0002, 0xd420_0000]);
    (words == expected).then_some(payload - 4)
}

#[cfg(test)]
mod tests {
    use super::{
        NativeTarget, decode_aarch64_selected_hosted_read_byte, effects,
        encode_aarch64_selected_hosted_read_byte_form,
        validate_aarch64_selected_hosted_read_byte_form,
    };
    use crate::test_support::reject_every_bit_flip;
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey,
        MachineEncodedStackEffect, MachineSemanticKind, MachineSizeKnowledge,
        SelectedInstructionKind,
    };
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn read_byte_replays_every_bit_and_rejects_wrong_home_source_slot_and_alternative() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let operation = OperationId::new(7).unwrap();
        let kind = SelectedInstructionKind::HostedReadByte {
            slot: LocalStorageSlotId::Structural {
                operation,
                place: PlaceId::new(1).unwrap(),
            },
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::HostedReadByte,
            variant: 0,
        };
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            // Inside the first 4 KiB, at the shifted boundary, past it with and
            // without a remainder, past 16 MiB in further page steps, and at
            // the last home whose payload a `u32` frame can hold.
            for displacement in [
                0,
                32,
                4088,
                4092,
                4096,
                1_048_572,
                900_000,
                16_777_208,
                16_777_212,
                33_554_428,
                u32::MAX - 7,
            ] {
                let encoded = encode_aarch64_selected_hosted_read_byte_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &[],
                    i64::from(displacement),
                )
                .unwrap();
                assert_eq!(
                    decode_aarch64_selected_hosted_read_byte(target, encoded.bytes()),
                    Some(displacement)
                );
                assert_eq!(encoded.footprint().encoded, effects(target));
                assert!(encoded.footprint().register_reads.is_empty());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
                reject_every_bit_flip(encoded.bytes(), "hosted read byte", |changed| {
                    assert_ne!(
                        decode_aarch64_selected_hosted_read_byte(target, &changed),
                        Some(displacement),
                        "decoder bit"
                    );
                    validate_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[],
                        i64::from(displacement),
                        &changed,
                    )
                    .is_err()
                });
                for invalid_offset in [1, 3, u32::MAX - 3, u32::MAX] {
                    assert!(
                        encode_aarch64_selected_hosted_read_byte_form(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &[],
                            i64::from(invalid_offset)
                        )
                        .is_err()
                    );
                }
                assert!(
                    validate_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[],
                        i64::from(displacement ^ 4),
                        encoded.bytes()
                    )
                    .is_err()
                );
                let boundary = SelectedInstructionKind::HostedReadByte {
                    slot: LocalStorageSlotId::Boundary { operation },
                };
                assert!(
                    encode_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        boundary,
                        alternative,
                        &[],
                        i64::from(displacement)
                    )
                    .is_err()
                );
                let source = [physical.model().view_named("x0").unwrap().id];
                assert!(
                    encode_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &source,
                        i64::from(displacement)
                    )
                    .is_err()
                );
                let wrong = MachineAlternativeKey {
                    variant: 1,
                    ..alternative
                };
                assert!(
                    encode_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        wrong,
                        &[],
                        i64::from(displacement)
                    )
                    .is_err()
                );
                assert!(
                    validate_aarch64_selected_hosted_read_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[],
                        i64::from(displacement),
                        &encoded.bytes()[1..]
                    )
                    .is_err()
                );
                let mut padded = encoded.bytes().to_vec();
                padded.push(0);
                assert_eq!(
                    decode_aarch64_selected_hosted_read_byte(target, &padded),
                    None
                );
                let other_target = if target == NativeTarget::linux_arm64() {
                    NativeTarget::macos_arm64()
                } else {
                    NativeTarget::linux_arm64()
                };
                assert!(
                    validate_aarch64_selected_hosted_read_byte_form(
                        other_target,
                        &physical,
                        kind,
                        alternative,
                        &[],
                        i64::from(displacement),
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert_eq!(
                    decode_aarch64_selected_hosted_read_byte(other_target, encoded.bytes()),
                    None
                );
            }
        }
    }

    #[test]
    fn read_byte_linux_golden_and_darwin_return_paths() {
        let linux = crate::encode_linux_read_byte_to_stack(16, 20).unwrap();
        let golden: [u32; 14] = [
            0x9100_53e1,
            0xb81f_c03f,
            0xb900_003f,
            0xd280_0000,
            0xd280_0022,
            0xd280_07e8,
            0xd400_0001,
            0xb400_00e0,
            0xf100_041f,
            0x5400_0081,
            0x5280_0029,
            0xb81f_c029,
            0x1400_0002,
            0xd420_0000,
        ];
        assert_eq!(
            linux,
            golden
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let darwin_words = |home: u32| {
            crate::encode_macos_read_byte_to_stack(home, home + 4)
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|word| u32::from_le_bytes(*word))
                .collect::<Vec<_>>()
        };
        let words = darwin_words(16);
        assert_eq!(&words[..3], &golden[..3]);
        assert_eq!(
            &words[3..13],
            &[
                0xd280_0000,
                0xd280_0022,
                0xd280_0070,
                0xd400_1001,
                0x5400_0102,
                0xb400_0100,
                0xf100_041f,
                0x5400_00a1,
                0x5280_0029,
                0x9100_53e1,
            ]
        );
        assert_eq!(&words[13..], &golden[11..]);
        // Past the first 4 KiB the address is a shifted add and its remainder.
        assert_eq!(&darwin_words(900_000)[..2], &[0x9143_6fe1, 0x912e_9021]);
        // Past 16 MiB the address continues in page steps from `x1`.
        assert_eq!(&darwin_words(33_554_428)[..2], &[0x917f_ffe1, 0x917f_fc21]);
        for (home, payload) in [(0, 0), (0, 8), (2, 6), (u32::MAX, 3)] {
            assert!(crate::encode_macos_read_byte_to_stack(home, payload).is_err());
            assert!(crate::encode_linux_read_byte_to_stack(home, payload).is_err());
        }
        // Execute the branch suffix from actual words, with injected syscall
        // results, for a one-word and a two-word payload address. In
        // particular errno 1 must never manufacture a Byte.
        for home in [16, 900_000] {
            let words = darwin_words(home);
            let address = &words[..if home < 4092 { 1 } else { 2 }];
            for carry in [false, true] {
                for count in [0, 1, 2, 4, u64::MAX] {
                    let mut position = address.len() + 6;
                    let mut zero = false;
                    let mut tag = 0;
                    let mut readdressed = false;
                    let mut trapped = false;
                    for _ in 0..10 {
                        if position == words.len() {
                            break;
                        }
                        let word = words[position];
                        if word & 0xff00_0010 == 0x5400_0000 {
                            let condition = word & 15;
                            let taken = match condition {
                                1 => !zero,
                                2 => carry,
                                _ => panic!("unexpected condition"),
                            };
                            position += if taken {
                                ((word >> 5) & 0x7ffff) as usize
                            } else {
                                1
                            };
                        } else if word & 0xff00_001f == 0xb400_0000 {
                            position += if count == 0 {
                                ((word >> 5) & 0x7ffff) as usize
                            } else {
                                1
                            };
                        } else if words[position..].starts_with(address) {
                            readdressed = true;
                            position += address.len();
                        } else {
                            match word {
                                0xf100_041f => {
                                    zero = count == 1;
                                    position += 1;
                                }
                                0x5280_0029 => {
                                    position += 1;
                                }
                                0xb81f_c029 => {
                                    assert!(readdressed, "tag stored before the address");
                                    tag = 1;
                                    position += 1;
                                }
                                0x1400_0002 => {
                                    position += (word & 0x03ff_ffff) as usize;
                                }
                                0xd420_0000 => {
                                    trapped = true;
                                    break;
                                }
                                _ => panic!("unexpected instruction"),
                            }
                        }
                    }
                    assert_eq!(trapped, carry || count > 1, "carry {carry}, count {count}");
                    if !trapped {
                        assert_eq!(position, words.len());
                        assert_eq!(tag, u32::from(count == 1));
                    }
                }
            }
        }
    }

    #[test]
    fn read_catalog_retains_complete_zero_operand_constraint_and_effect_clobbers() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let constraints = crate::validate_aarch64_register_constraint_catalog(
            crate::aarch64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
            let validated = crate::validate_aarch64_machine_effect_catalog(
                target,
                &constraints,
                catalog.clone(),
            )
            .unwrap();
            let declaration = validated
                .catalog()
                .declaration_by_semantic(MachineSemanticKind::HostedReadByte)
                .unwrap();
            let expected_key = if target == NativeTarget::linux_arm64() {
                crate::AARCH64_HOSTED_READ_BYTE
            } else {
                crate::AARCH64_DARWIN_HOSTED_READ_BYTE
            };
            assert_eq!(declaration.constraint, expected_key);
            assert_eq!(
                declaration.alternatives[0].size,
                MachineSizeKnowledge::EncoderResolved {
                    minimum_bytes: if target == NativeTarget::linux_arm64() {
                        56
                    } else {
                        64
                    },
                    // 258 address words past the fixed sequence; Darwin
                    // forms the address twice.
                    maximum_bytes: Some(if target == NativeTarget::linux_arm64() {
                        52 + 1_032
                    } else {
                        56 + 2 * 1_032
                    }),
                }
            );
            assert_eq!(declaration.alternatives[0].encoded, effects(target));
            let mut damaged = catalog;
            let row = damaged
                .declaration_by_semantic_mut(MachineSemanticKind::HostedReadByte)
                .unwrap();
            row.alternatives[0].encoded.implicit_unit_clobbers.pop();
            assert!(
                crate::validate_aarch64_machine_effect_catalog(target, &constraints, damaged)
                    .is_err()
            );
        }
    }
}
