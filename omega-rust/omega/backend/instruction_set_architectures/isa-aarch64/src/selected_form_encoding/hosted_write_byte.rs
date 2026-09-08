//! Hosted write-one-byte leaf: caller frame storage, no hidden stack adjustment.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;
use target::NativeTarget;

pub(crate) fn effects(target: NativeTarget) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical hosted write register")
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
    for name in ["x0", "x1", "x2", syscall_register, "nzcv"] {
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
        memory: MachineEncodedMemoryEffect::HostedWriteByteV1 {
            stack_pointer: view("sp").id,
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::HostedWriteFailureV1,
        control: MachineEncodedControlEffect::HostedWriteReturnOrTrapV1,
    }
}

pub(crate) fn declaration(
    target: NativeTarget,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::HostedWriteByteI32,
        constraint,
        memory: MachineMemoryEffect::HostedWriteByteV1,
        trap: MachineTrapBehavior::HostedWriteFailureV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedWriteByteI32,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(if target == NativeTarget::macos_arm64() {
                40
            } else {
                36
            }),
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
    displacement: u32,
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
        || physical.model() != &crate::aarch64_physical_register_model()
        || !matches!(
            kind,
            SelectedInstructionKind::HostedWriteByteI32 {
                slot: LocalStorageSlotId::Boundary { .. }
            }
        )
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedWriteByteI32,
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

pub fn encode_aarch64_selected_hosted_write_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands, displacement)?;
    let mut words = vec![
        0x3900_03e0 | (displacement << 10) | u32::from(register),
        0xd280_0020,
        0x9100_03e1 | (displacement << 10),
        0xd280_0022,
    ];
    if target == NativeTarget::macos_arm64() {
        // Darwin returns errors via carry. Test it before CMP overwrites NZCV.
        // Apple ABI: x16 selects write(4), SVC #0x80 enters the kernel.
        // https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/custom/SYS.h
        // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/syscalls.master
        words.extend([
            0xd280_0090,
            0xd400_1001,
            0x5400_0062,
            0xf100_041f,
            0x5400_0040,
            0xd420_0000,
        ]);
    } else {
        words.extend([
            0xd280_0808,
            0xd400_0001,
            0xf100_001f,
            0x5400_004c,
            0xd420_0000,
        ]);
    }
    let bytes = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    validate_aarch64_selected_hosted_write_byte_form(
        target,
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

pub fn validate_aarch64_selected_hosted_write_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands, displacement)?;
    if decode(target, bytes) != Some((register, displacement)) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands.to_vec(),
            register_writes: Vec::new(),
            writes_nzcv: true,
            encoded: effects(target),
        },
    })
}

fn decode(target: NativeTarget, bytes: &[u8]) -> Option<(u8, u32)> {
    let suffix: &[u32] = if target == NativeTarget::linux_arm64() {
        &[
            0xd280_0022,
            0xd280_0808,
            0xd400_0001,
            0xf100_001f,
            0x5400_004c,
            0xd420_0000,
        ]
    } else if target == NativeTarget::macos_arm64() {
        &[
            0xd280_0022,
            0xd280_0090,
            0xd400_1001,
            0x5400_0062,
            0xf100_041f,
            0x5400_0040,
            0xd420_0000,
        ]
    } else {
        return None;
    };
    if bytes.len() != (3 + suffix.len()) * 4 {
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
        || &words[3..] != suffix
    {
        return None;
    }
    Some(((words[0] & 31) as u8, (words[0] >> 10) & 4095))
}

/// Independently decode the complete returning write leaf for object custody replay.
pub fn decode_aarch64_selected_hosted_write_byte_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<(calling_conventions::MachineRegister, u32)> {
    let (register, displacement) = decode(target, bytes)?;
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
    fn hosted_write_byte_replays_source_frame_offset_syscall_failure_branch_and_every_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let kind = SelectedInstructionKind::HostedWriteByteI32 {
            slot: LocalStorageSlotId::Boundary {
                operation: OperationId::new(7).unwrap(),
            },
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::HostedWriteByteI32,
            variant: 0,
        };
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            for name in ["x0", "x1", "x2", "x8", "x9", "x16", "x28"] {
                let operands = [physical.model().view_named(name).unwrap().id];
                for displacement in [0, 31, 4095] {
                    let encoded = encode_aarch64_selected_hosted_write_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                    )
                    .unwrap();
                    assert_eq!(encoded.footprint().encoded, effects(target));
                    let other_target = if target == NativeTarget::linux_arm64() {
                        NativeTarget::macos_arm64()
                    } else {
                        NativeTarget::linux_arm64()
                    };
                    assert!(decode(other_target, encoded.bytes()).is_none());
                    assert!(decode(NativeTarget::linux_x64(), encoded.bytes()).is_none());
                    assert!(
                        encode_aarch64_selected_hosted_write_byte_form(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                        )
                        .is_err()
                    );
                    assert!(encoded.footprint().register_writes.is_empty());
                    assert_eq!(
                        encoded.footprint().encoded.stack,
                        MachineEncodedStackEffect::UnchangedV1
                    );
                    for bit in 0..encoded.bytes().len() * 8 {
                        let mut changed = encoded.bytes().to_vec();
                        changed[bit / 8] ^= 1 << (bit % 8);
                        assert!(
                            validate_aarch64_selected_hosted_write_byte_form(
                                target,
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
                        validate_aarch64_selected_hosted_write_byte_form(
                            target,
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
                        validate_aarch64_selected_hosted_write_byte_form(
                            target,
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
                        validate_aarch64_selected_hosted_write_byte_form(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            displacement,
                            &trailing
                        )
                        .is_err()
                    );
                    let wrong_slot = SelectedInstructionKind::HostedWriteByteI32 {
                        slot: LocalStorageSlotId::Structural {
                            operation: OperationId::new(7).unwrap(),
                            place: PlaceId::new(1).unwrap(),
                        },
                    };
                    assert!(
                        validate_aarch64_selected_hosted_write_byte_form(
                            target,
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
                        encode_aarch64_selected_hosted_write_byte_form(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            4096
                        )
                        .is_err()
                    );
                    assert!(
                        encode_aarch64_selected_hosted_write_byte_form(
                            target,
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

    #[test]
    fn darwin_success_requires_clear_carry_and_exactly_one_byte() {
        let words = [
            0x3900_03e9_u32,
            0xd280_0020,
            0x9100_03e1,
            0xd280_0022,
            0xd280_0090,
            0xd400_1001,
            0x5400_0062,
            0xf100_041f,
            0x5400_0040,
            0xd420_0000,
        ];
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(decode(NativeTarget::macos_arm64(), &bytes), Some((9, 0)));
        for (word, replacement) in [
            (6, 0xd503_201f_u32), // Dropping the carry test may accept positive errno.
            (7, 0xf100_001f),     // CMP zero instead of the requested byte count.
            (8, 0x5400_004c),     // B.GT instead of exact equality.
        ] {
            let mut changed = bytes.clone();
            changed[word * 4..word * 4 + 4].copy_from_slice(&replacement.to_le_bytes());
            assert!(decode(NativeTarget::macos_arm64(), &changed).is_none());
        }
    }

    #[test]
    fn target_effects_bind_the_exact_syscall_register() {
        let physical = crate::aarch64_physical_register_model();
        for (target, used, preserved) in [
            (NativeTarget::linux_arm64(), "x8", "x16"),
            (NativeTarget::macos_arm64(), "x16", "x8"),
        ] {
            let effects = effects(target);
            assert!(
                physical
                    .view_named(used)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| effects.implicit_unit_clobbers.contains(unit))
            );
            assert!(
                physical
                    .view_named(preserved)
                    .unwrap()
                    .units
                    .iter()
                    .all(|unit| !effects.implicit_unit_clobbers.contains(unit))
            );
        }
    }
}
