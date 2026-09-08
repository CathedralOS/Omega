//! Exact hosted exit: normalize the low i32 carrier, invoke the kernel, trap if it returns.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;
use target::NativeTarget;

pub(crate) fn effects(
    target: NativeTarget,
) -> Result<MachineEncodedEffects, X86_64SelectedFormEncodingError> {
    let physical = crate::x86_64_physical_register_model();
    let view = |name: &str| {
        physical
            .view_named(name)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)
    };
    let mut clobbers = Vec::new();
    let _ = target;
    for name in ["rax", "rdi", "rcx", "r11", "rflags"] {
        clobbers.extend(view(name)?.units.iter().copied());
    }
    clobbers.sort_unstable();
    clobbers.dedup();
    Ok(MachineEncodedEffects {
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: view("rip")?.units.clone(),
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
) -> Result<MachineEffectDeclaration, X86_64SelectedFormEncodingError> {
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
            size: MachineSizeKnowledge::ExactBytes(12),
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
) -> Result<u8, X86_64SelectedFormEncodingError> {
    if target != NativeTarget::linux_x64()
        || physical.model() != &crate::x86_64_physical_register_model()
        || kind != SelectedInstructionKind::HostedExitProcessI32
        || alternative
            != (MachineAlternativeKey {
                family: MachineAlternativeFamily::HostedExitProcessI32,
                variant: 0,
            })
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let [register] = registers.as_slice() else {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    };
    Ok(*register)
}

pub fn encode_x86_64_selected_hosted_exit_process_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands)?;
    let bytes = vec![
        0x40 | ((register >> 3) << 2),
        0x89,
        0xc7 | ((register & 7) << 3),
        0xb8,
        0xe7,
        0,
        0,
        0,
        0x0f,
        0x05,
        0x0f,
        0x0b,
    ];
    validate_x86_64_selected_hosted_exit_process_form(
        target,
        physical,
        kind,
        alternative,
        operands,
        &bytes,
    )
}

pub fn validate_x86_64_selected_hosted_exit_process_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands)?;
    if decode(target, bytes) != Some(register) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands.to_vec(),
            register_writes: Vec::new(),
            writes_rflags: true,
            encoded: effects(target)?,
        },
    })
}

/// Decode every opcode and fixed immediate independently of the producer.
fn decode(target: NativeTarget, bytes: &[u8]) -> Option<u8> {
    if target != NativeTarget::linux_x64() || bytes.len() != 12 {
        return None;
    }
    let register = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
    if bytes[0] & 0xfb != 0x40
        || bytes[1] != 0x89
        || bytes[2] & 0xc7 != 0xc7
        || register == 4
        || bytes[3..] != [0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b]
    {
        return None;
    }
    Some(register)
}

/// Source-free decoding retains the actual input home, not merely the syscall.
pub fn decode_x86_64_selected_hosted_exit_process_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<calling_conventions::MachineRegister> {
    let register = decode(target, bytes)?;
    use calling_conventions::MachineRegister::*;
    let registers = [
        X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
        X86R11, X86R12, X86R13, X86R14, X86R15,
    ];
    Some(registers[usize::from(register)])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_exit_replays_target_input_and_every_encoded_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let kind = SelectedInstructionKind::HostedExitProcessI32;
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::HostedExitProcessI32,
            variant: 0,
        };
        for target in [NativeTarget::linux_x64()] {
            for name in ["rax", "rdi", "rcx", "r8", "r11", "r15"] {
                let operands = [physical.model().view_named(name).unwrap().id];
                let encoded = encode_x86_64_selected_hosted_exit_process_form(
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
                        validate_x86_64_selected_hosted_exit_process_form(
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
                    .view_named(if name == "rax" { "rdi" } else { "rax" })
                    .unwrap()
                    .id];
                assert!(
                    validate_x86_64_selected_hosted_exit_process_form(
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
                    encode_x86_64_selected_hosted_exit_process_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[]
                    )
                    .is_err()
                );
                assert!(
                    encode_x86_64_selected_hosted_exit_process_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &[operands[0], operands[0]]
                    )
                    .is_err()
                );
                assert!(
                    encode_x86_64_selected_hosted_exit_process_form(
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
                    NativeTarget::macos_arm64(),
                ] {
                    assert!(
                        validate_x86_64_selected_hosted_exit_process_form(
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
                assert!(decode_x86_64_selected_hosted_exit_process_i32(target, &extra).is_none());
            }
        }
    }
}
