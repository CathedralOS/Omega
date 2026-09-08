//! Exact hosted exit: normalize the low i32 carrier, invoke the kernel, trap if it returns.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;
use target::NativeTarget;

pub(crate) fn effects(
    target: NativeTarget,
) -> Result<MachineEncodedEffects, Aarch64SelectedFormEncodingError> {
    let physical = crate::aarch64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .ok_or(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    };
    let mut clobbers = Vec::new();
    let syscall_register = if target == NativeTarget::macos_arm64() {
        "x16"
    } else {
        "x8"
    };
    for name in ["x0", syscall_register, "nzcv"] {
        clobbers.extend(view(name)?.units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    Ok(MachineEncodedEffects {
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: view("pc")?.units.clone(),
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::HostedExitReturnedV1,
        control: MachineEncodedControlEffect::HostedExitOrTrapV1,
    })
}

pub(crate) fn declaration(
    target: NativeTarget,
    constraint: RegisterConstraintKey,
) -> Result<MachineEffectDeclaration, Aarch64SelectedFormEncodingError> {
    Ok(MachineEffectDeclaration {
        semantic: MachineSemanticKind::HostedExitProcessI32,
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::HostedExitReturnedV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedExitProcessI32,
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(16),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(target)?,
        }],
    })
}

fn request(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<u8, Aarch64SelectedFormEncodingError> {
    if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
        || physical.model() != &crate::aarch64_physical_register_model()
        || kind != SelectedInstructionKind::HostedExitProcessI32
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedExitProcessI32,
                variant: 0,
            })
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let [register] = registers.as_slice() else {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    };
    Ok(*register)
}

pub fn encode_aarch64_selected_hosted_exit_process_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands)?;
    let words = [
        0x2a00_03e0 | (u32::from(register) << 16),
        if target == NativeTarget::macos_arm64() {
            0xd280_0030
        } else {
            0xd280_0bc8
        },
        if target == NativeTarget::macos_arm64() {
            0xd400_1001
        } else {
            0xd400_0001
        },
        0xd420_0000,
    ];
    let bytes = words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    validate_aarch64_selected_hosted_exit_process_form(
        target,
        physical,
        kind,
        alternative,
        operands,
        &bytes,
    )
}

pub fn validate_aarch64_selected_hosted_exit_process_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands)?;
    if decode(target, bytes) != Some(register) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: operands.to_vec(),
            register_writes: Vec::new(),
            writes_nzcv: true,
            encoded: effects(target)?,
        },
    })
}

/// Decode every opcode and fixed immediate independently of the producer.
fn decode(target: NativeTarget, bytes: &[u8]) -> Option<u8> {
    if ![NativeTarget::linux_arm64(), NativeTarget::macos_arm64()].contains(&target)
        || bytes.len() != 16
    {
        return None;
    }
    let words = [
        u32::from_le_bytes(bytes[0..4].try_into().ok()?),
        u32::from_le_bytes(bytes[4..8].try_into().ok()?),
        u32::from_le_bytes(bytes[8..12].try_into().ok()?),
        u32::from_le_bytes(bytes[12..16].try_into().ok()?),
    ];
    let register = ((words[0] >> 16) & 31) as u8;
    if words[0] & !0x001f_0000 != 0x2a00_03e0
        || register == 31
        || words[1]
            != if target == NativeTarget::macos_arm64() {
                0xd280_0030
            } else {
                0xd280_0bc8
            }
        || words[2]
            != if target == NativeTarget::macos_arm64() {
                0xd400_1001
            } else {
                0xd400_0001
            }
        || words[3] != 0xd420_0000
    {
        return None;
    }
    Some(register)
}

/// Source-free decoding retains the actual input home, not merely the syscall.
pub fn decode_aarch64_selected_hosted_exit_process_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<calling_conventions::MachineRegister> {
    let register = decode(target, bytes)?;
    Some(calling_conventions::MachineRegister::Aarch64X(register))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_exit_replays_target_input_and_every_encoded_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
        )
        .unwrap();
        let kind = SelectedInstructionKind::HostedExitProcessI32;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::HostedExitProcessI32,
            variant: 0,
        };
        for target in [NativeTarget::linux_arm64(), NativeTarget::macos_arm64()] {
            for name in ["x0", "x1", "x8", "x16", "x18", "x30"] {
                let operands = [physical.model().view_named(name).unwrap().id];
                let encoded = encode_aarch64_selected_hosted_exit_process_form(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                )
                .unwrap();
                assert_eq!(encoded.footprint().encoded, effects(target).unwrap());
                for bit in 0..encoded.bytes().len() * 8 {
                    let mut corrupted = encoded.bytes().to_vec();
                    corrupted[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate_aarch64_selected_hosted_exit_process_form(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &corrupted
                        )
                        .is_err(),
                        "bit {bit}"
                    );
                }
                let different = [physical
                    .model()
                    .view_named(if name == "x0" { "x1" } else { "x0" })
                    .unwrap()
                    .id];
                assert!(
                    validate_aarch64_selected_hosted_exit_process_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &different,
                        encoded.bytes()
                    )
                    .is_err()
                );
                assert!(
                    encode_aarch64_selected_hosted_exit_process_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[]
                    )
                    .is_err()
                );
                assert!(
                    encode_aarch64_selected_hosted_exit_process_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[operands[0], operands[0]]
                    )
                    .is_err()
                );
                assert!(
                    encode_aarch64_selected_hosted_exit_process_form(
                        target,
                        &physical,
                        SelectedInstructionKind::ReturnUnit,
                        alternative,
                        &operands
                    )
                    .is_err()
                );
                for wrong_target in [
                    NativeTarget::windows_x64(),
                    NativeTarget {
                        pointer_size: 4,
                        ..target
                    },
                    if target == NativeTarget::linux_arm64() {
                        NativeTarget::macos_arm64()
                    } else {
                        NativeTarget::linux_arm64()
                    },
                ] {
                    assert!(
                        validate_aarch64_selected_hosted_exit_process_form(
                            wrong_target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            encoded.bytes()
                        )
                        .is_err()
                    );
                }
                let mut extra = encoded.bytes().to_vec();
                extra.push(0);
                assert!(decode_aarch64_selected_hosted_exit_process_i32(target, &extra).is_none());
            }
        }
    }
}
