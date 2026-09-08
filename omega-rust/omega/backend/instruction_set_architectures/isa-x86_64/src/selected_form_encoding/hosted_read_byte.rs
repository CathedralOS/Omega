//! Hosted read-one-byte leaf: caller frame storage, no hidden stack adjustment.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;

pub(crate) fn effects() -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical Linux read register")
    };
    let mut uses = view("rsp").units.clone();
    uses.extend(view("rip").units.iter().copied());
    uses.sort_unstable();
    uses.dedup();
    let mut clobbers = Vec::new();
    for name in ["rax", "rdi", "rsi", "rdx", "rcx", "r11", "rflags"] {
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
            stack_pointer: view("rsp").id,
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
            size: MachineSizeKnowledge::ExactBytes(62),
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
) -> Result<(), X86_64SelectedFormEncodingError> {
    if physical.model() != &crate::x86_64_physical_register_model()
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
        || displacement > 2147483640
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(())
}

/// Encode a Linux read into an eight-byte caller-owned structural home.
pub fn encode_x86_64_selected_hosted_read_byte_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    request(physical, kind, alternative, operands, displacement)?;
    let bytes = crate::encode_linux_read_byte_to_stack(displacement, displacement + 4)
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    validate_x86_64_selected_hosted_read_byte_form(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

/// Replay every encoded instruction and address independently of the encoder.
pub fn validate_x86_64_selected_hosted_read_byte_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    request(physical, kind, alternative, operands, displacement)?;
    if decode_x86_64_selected_hosted_read_byte(bytes) != Some(displacement) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            writes_rflags: true,
            encoded: effects(),
        },
    })
}

/// Decode the complete Linux read leaf, returning its structural home byte offset.
pub fn decode_x86_64_selected_hosted_read_byte(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 62 {
        return None;
    }
    let home = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    let payload = home.checked_add(4)?;
    if !home.is_multiple_of(4)
        || home > 2147483640
        || bytes[..5] != [0x31, 0xc0, 0x89, 0x84, 0x24]
        || bytes[9..12] != [0x89, 0x84, 0x24]
        || u32::from_le_bytes(bytes[12..16].try_into().ok()?) != payload
        || bytes[16..22] != [0x31, 0xff, 0x48, 0x8d, 0xb4, 0x24]
        || u32::from_le_bytes(bytes[22..26].try_into().ok()?) != payload
        || bytes[26..50]
            != [
                0xba, 1, 0, 0, 0, 0x31, 0xc0, 0x0f, 0x05, 0x48, 0x83, 0xf8, 0, 0x74, 21, 0x48,
                0x83, 0xf8, 1, 0x75, 13, 0xc7, 0x84, 0x24,
            ]
        || u32::from_le_bytes(bytes[50..54].try_into().ok()?) != home
        || bytes[54..] != [1, 0, 0, 0, 0xeb, 2, 0x0f, 0x0b]
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
            crate::x86_64_physical_register_model(),
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
        for displacement in [0, 32, 2147483640] {
            let encoded = encode_x86_64_selected_hosted_read_byte_form(
                &physical,
                kind,
                alternative,
                &[],
                displacement,
            )
            .unwrap();
            assert_eq!(
                decode_x86_64_selected_hosted_read_byte(encoded.bytes()),
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
                    validate_x86_64_selected_hosted_read_byte_form(
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
                    decode_x86_64_selected_hosted_read_byte(&changed),
                    None,
                    "decoder bit {bit}"
                );
            }
            for invalid_offset in [1, 3, 2147483644, u32::MAX] {
                assert!(
                    encode_x86_64_selected_hosted_read_byte_form(
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
                validate_x86_64_selected_hosted_read_byte_form(
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
                encode_x86_64_selected_hosted_read_byte_form(
                    &physical,
                    boundary,
                    alternative,
                    &[],
                    displacement
                )
                .is_err()
            );
            let source = [physical.model().view_named("rax").unwrap().id];
            assert!(
                encode_x86_64_selected_hosted_read_byte_form(
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
                encode_x86_64_selected_hosted_read_byte_form(
                    &physical,
                    kind,
                    wrong,
                    &[],
                    displacement
                )
                .is_err()
            );
            assert!(
                validate_x86_64_selected_hosted_read_byte_form(
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
            assert_eq!(decode_x86_64_selected_hosted_read_byte(&padded), None);
        }
    }

    #[test]
    fn read_catalog_retains_complete_zero_operand_constraint_and_effect_clobbers() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let constraints = crate::validate_x86_64_register_constraint_catalog(
            crate::x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        let target = target::NativeTarget::linux_x64();
        let catalog = crate::x86_64_machine_effect_catalog(target, &constraints).unwrap();
        let validated =
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, catalog.clone())
                .unwrap();
        let declaration = validated
            .catalog()
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::HostedReadByte)
            .unwrap();
        assert_eq!(declaration.constraint, crate::X86_64_HOSTED_READ_BYTE);
        assert_eq!(declaration.alternatives[0].encoded, effects());
        let mut damaged = catalog;
        let row = damaged
            .declarations
            .iter_mut()
            .find(|row| row.semantic == MachineSemanticKind::HostedReadByte)
            .unwrap();
        row.alternatives[0].encoded.implicit_unit_clobbers.pop();
        assert!(
            crate::validate_x86_64_machine_effect_catalog(target, &constraints, damaged).is_err()
        );
    }
}
