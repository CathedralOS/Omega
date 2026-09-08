//! Hosted read-one-byte leaf: caller frame storage, no hidden stack adjustment.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;
use target::NativeTarget;

pub(crate) fn effects() -> MachineEncodedEffects {
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
    for name in ["x0", "x1", "x2", "x8", "x9", "nzcv"] {
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

pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
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
            size: MachineSizeKnowledge::ExactBytes(56),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(),
        }],
    }
}

fn request(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<(), Aarch64SelectedFormEncodingError> {
    if target != NativeTarget::linux_arm64()
        || physical.model() != &crate::aarch64_physical_register_model()
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
        || !displacement.is_multiple_of(4)
        || displacement > 4088
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

/// Encode a Linux read into an eight-byte caller-owned structural home.
pub fn encode_aarch64_selected_hosted_read_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    request(target, physical, kind, alternative, operands, displacement)?;
    let bytes = crate::encode_linux_read_byte_to_stack(displacement, displacement + 4)
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    validate_aarch64_selected_hosted_read_byte_form(
        target,
        physical,
        kind,
        alternative,
        operands,
        displacement,
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
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    request(target, physical, kind, alternative, operands, displacement)?;
    if decode_aarch64_selected_hosted_read_byte(target, bytes) != Some(displacement) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            writes_nzcv: true,
            encoded: effects(),
        },
    })
}

/// Decode the complete Linux read leaf, returning its structural home byte offset.
pub fn decode_aarch64_selected_hosted_read_byte(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
    if target != NativeTarget::linux_arm64() || bytes.len() != 56 {
        return None;
    }
    let words = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    let home = ((words[0] >> 10) & 4095) * 4;
    let payload = home.checked_add(4)?;
    if home > 4088
        || words[0] & 0xffc0_03ff != 0xb900_03ff
        || words[1] & 0xffc0_03ff != 0xb900_03ff
        || ((words[1] >> 10) & 4095) * 4 != payload
        || words[2] & 0xffc0_03ff != 0x9100_03e1
        || (words[2] >> 10) & 4095 != payload
        || words[3..11]
            != [
                0xd280_0000,
                0xd280_0022,
                0xd280_07e8,
                0xd400_0001,
                0xb400_00e0,
                0xf100_041f,
                0x5400_0081,
                0x5280_0029,
            ]
        || words[11] & 0xffc0_03ff != 0xb900_03e9
        || ((words[11] >> 10) & 4095) * 4 != home
        || words[12..] != [0x1400_0002, 0xd420_0000]
    {
        return None;
    }
    Some(home)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        for displacement in [0, 32, 4088] {
            let encoded = encode_aarch64_selected_hosted_read_byte_form(
                target::NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &[],
                displacement,
            )
            .unwrap();
            assert_eq!(
                decode_aarch64_selected_hosted_read_byte(
                    target::NativeTarget::linux_arm64(),
                    encoded.bytes()
                ),
                Some(displacement)
            );
            assert_eq!(encoded.footprint().encoded, effects());
            assert!(encoded.footprint().register_reads.is_empty());
            assert!(encoded.footprint().register_writes.is_empty());
            assert_eq!(
                encoded.footprint().encoded.stack,
                MachineEncodedStackEffect::UnchangedV1
            );
            for bit in 0..encoded.bytes().len() * 8 {
                let mut changed = encoded.bytes().to_vec();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert!(
                    validate_aarch64_selected_hosted_read_byte_form(
                        target::NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement,
                        &changed
                    )
                    .is_err(),
                    "bit {bit}"
                );
                assert_eq!(
                    decode_aarch64_selected_hosted_read_byte(
                        target::NativeTarget::linux_arm64(),
                        &changed
                    ),
                    None,
                    "decoder bit {bit}"
                );
            }
            for invalid_offset in [1, 3, 4092, u32::MAX] {
                assert!(
                    encode_aarch64_selected_hosted_read_byte_form(
                        target::NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &[],
                        invalid_offset
                    )
                    .is_err()
                );
            }
            assert!(
                validate_aarch64_selected_hosted_read_byte_form(
                    target::NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &[],
                    displacement ^ 4,
                    encoded.bytes()
                )
                .is_err()
            );
            let boundary = SelectedInstructionKind::HostedReadByte {
                slot: LocalStorageSlotId::Boundary { operation },
            };
            assert!(
                encode_aarch64_selected_hosted_read_byte_form(
                    target::NativeTarget::linux_arm64(),
                    &physical,
                    boundary,
                    alternative,
                    &[],
                    displacement
                )
                .is_err()
            );
            let source = [physical.model().view_named("x0").unwrap().id];
            assert!(
                encode_aarch64_selected_hosted_read_byte_form(
                    target::NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &source,
                    displacement
                )
                .is_err()
            );
            let wrong = MachineAlternativeKey {
                variant: 1,
                ..alternative
            };
            assert!(
                encode_aarch64_selected_hosted_read_byte_form(
                    target::NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    wrong,
                    &[],
                    displacement
                )
                .is_err()
            );
            assert!(
                validate_aarch64_selected_hosted_read_byte_form(
                    target::NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &[],
                    displacement,
                    &encoded.bytes()[1..]
                )
                .is_err()
            );
            let mut padded = encoded.bytes().to_vec();
            padded.push(0);
            assert_eq!(
                decode_aarch64_selected_hosted_read_byte(
                    target::NativeTarget::linux_arm64(),
                    &padded
                ),
                None
            );
            assert!(
                encode_aarch64_selected_hosted_read_byte_form(
                    target::NativeTarget::macos_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &[],
                    displacement
                )
                .is_err()
            );
            assert_eq!(
                decode_aarch64_selected_hosted_read_byte(
                    target::NativeTarget::macos_arm64(),
                    encoded.bytes()
                ),
                None
            );
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
        let target = target::NativeTarget::linux_arm64();
        let catalog = crate::aarch64_machine_effect_catalog(target, &constraints).unwrap();
        let validated =
            crate::validate_aarch64_machine_effect_catalog(target, &constraints, catalog.clone())
                .unwrap();
        let declaration = validated
            .catalog()
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::HostedReadByte)
            .unwrap();
        assert_eq!(declaration.constraint, crate::AARCH64_HOSTED_READ_BYTE);
        assert_eq!(declaration.alternatives[0].encoded, effects());
        let mut damaged = catalog;
        let row = damaged
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::HostedReadByte)
            .unwrap();
        row.alternatives[0].encoded.implicit_unit_clobbers.pop();
        assert!(
            crate::validate_aarch64_machine_effect_catalog(target, &constraints, damaged).is_err()
        );
    }
}
