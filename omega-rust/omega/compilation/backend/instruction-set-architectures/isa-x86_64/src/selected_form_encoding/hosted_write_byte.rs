//! Hosted write-one-byte leaf: caller frame storage, no hidden stack adjustment.
//! Linux and Intel macOS share the leaf; Darwin enters the Unix-class
//! `write` and adds one `jc` that traps on the carry-reported failure.
use super::{
    RegisterViewId, ValidatedPhysicalRegisterModel, ValidatedX86_64SelectedFormEncoding,
    X86_64SelectedFormEncodingError, X86_64SelectedFormFootprint,
};
use crate::selected_form_encoding::request_validation::resolve_registers;
use register_model::RegisterConstraintKey;
use selected_instructions::{
    LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, SelectedInstructionKind,
};
use target::NativeTarget;

/// Exact leaf width: 40 bytes on Linux, 42 with Darwin's carry branch.
fn leaf_bytes(target: NativeTarget) -> u16 {
    if crate::hosted_kernel::hosted_kernel(target)
        .is_some_and(|kernel| kernel.carry_reports_failure)
    {
        42
    } else {
        40
    }
}

pub(crate) fn effects() -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model_ref();
    let view = |name: &str| {
        physical
            .view_named(name)
            .expect("canonical Linux write register")
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
        external_operand_reads: vec![0],
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: Vec::new(),
        implicit_unit_clobbers: clobbers,
        memory: MachineEncodedMemoryEffect::HostedWriteByteV1 {
            stack_pointer: view("rsp").id,
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
    super::hosted_alternative_declaration(
        constraint,
        MachineSemanticKind::HostedWriteByteI32,
        MachineMemoryEffect::HostedWriteByteV1,
        MachineTrapBehavior::HostedWriteFailureV1,
        MachineAlternativeFamily::HostedWriteByteI32,
        MachineSizeKnowledge::ExactBytes(leaf_bytes(target)),
        effects(),
    )
}

fn request(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<u8, X86_64SelectedFormEncodingError> {
    // Hosted boundary storage is committed caller frame space: its
    // displacement is always a nonnegative offset below the incoming args.
    if !crate::hosted_syscall_target(target)
        || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
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
        || !(0..=i64::from(i32::MAX)).contains(&displacement)
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let registers = resolve_registers(physical, operands)?;
    let [register] = registers.as_slice() else {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    };
    Ok(*register)
}

pub fn encode_x86_64_selected_hosted_write_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands, displacement)?;
    let kernel = crate::hosted_kernel::hosted_kernel(target)
        .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    let displacement = i32::try_from(displacement)
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    let mut bytes = vec![
        0x40 | ((register >> 3) << 2),
        0x88,
        0x84 | ((register & 7) << 3),
        0x24,
    ];
    crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]);
    crate::codec_bytes::push_encoded(&mut bytes, displacement.to_le_bytes());
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0, 0xb8]);
    crate::codec_bytes::push_encoded(&mut bytes, kernel.write.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]);
    if kernel.carry_reports_failure {
        // jc ud2: Darwin's positive errno must not reach the count test.
        bytes.extend_from_slice(&[0x72, 0x05]);
    }
    bytes.extend_from_slice(&[0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f, 0x0b]);
    validate_x86_64_selected_hosted_write_byte_form(
        target,
        physical,
        kind,
        alternative,
        operands,
        i64::from(displacement),
        &bytes,
    )
}

pub fn validate_x86_64_selected_hosted_write_byte_form(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let register = request(target, physical, kind, alternative, operands, displacement)?;
    if decode(target, bytes).map(|(register, offset)| (register, i64::from(offset)))
        != Some((register, displacement))
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: operands.to_vec(),
            register_writes: Vec::new(),
            writes_rflags: true,
            encoded: effects(),
        },
    })
}

fn decode(target: NativeTarget, bytes: &[u8]) -> Option<(u8, u32)> {
    // The syscall tail is the literal Linux or Darwin spelling, never
    // derived from the encoder's kernel table.
    let tail: &[u8] = if target == NativeTarget::linux_x64() {
        &[
            0xba, 1, 0, 0, 0, 0xb8, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f,
            0x0b,
        ]
    } else if target == NativeTarget::macos_x64() {
        &[
            0xba, 1, 0, 0, 0, 0xb8, 4, 0, 0, 2, 0x0f, 0x05, 0x72, 0x05, 0x48, 0x85, 0xc0, 0x7f,
            0x02, 0x0f, 0x0b,
        ]
    } else {
        return None;
    };
    if bytes.len() != 21 + tail.len() {
        return None;
    }
    let stored_offset = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let addressed_offset = u32::from_le_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]);
    let decoded_register = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
    if bytes[0] & 0xfb != 0x40
        || bytes[1] != 0x88
        || bytes[2] & 0xc7 != 0x84
        || bytes[3] != 0x24
        || decoded_register == 4
        || stored_offset > i32::MAX as u32
        || addressed_offset != stored_offset
        || bytes[8..17] != [0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]
        || bytes[21..] != *tail
    {
        return None;
    }
    Some((decoded_register, stored_offset))
}

/// Independently decode the complete returning write leaf for object custody replay.
pub fn decode_x86_64_selected_hosted_write_byte_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<(calling_conventions::MachineRegister, u32)> {
    let (register, displacement) = decode(target, bytes)?;
    use calling_conventions::MachineRegister::*;
    let registers = [
        X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
        X86R11, X86R12, X86R13, X86R14, X86R15,
    ];
    Some((registers[usize::from(register)], displacement))
}

#[cfg(test)]
mod tests {
    use super::{
        NativeTarget, decode_x86_64_selected_hosted_write_byte_i32, effects,
        encode_x86_64_selected_hosted_write_byte_form,
        validate_x86_64_selected_hosted_write_byte_form,
    };
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeFamily, MachineAlternativeKey,
        MachineEncodedStackEffect, SelectedInstructionKind,
    };
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn hosted_write_byte_replays_source_frame_offset_syscall_failure_branch_and_every_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
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
        for (target, other_kernel, width) in [
            (NativeTarget::linux_x64(), NativeTarget::macos_x64(), 40),
            (NativeTarget::macos_x64(), NativeTarget::linux_x64(), 42),
        ] {
            for name in ["rax", "rbp", "rsi", "rdx", "r11", "r15"] {
                let operands = [physical.model().view_named(name).unwrap().id];
                for displacement in [0, 31, 2147483647] {
                    let encoded = encode_x86_64_selected_hosted_write_byte_form(
                        target,
                        &physical,
                        kind,
                        alternative,
                        &operands,
                        displacement,
                    )
                    .unwrap();
                    assert_eq!(encoded.footprint().encoded, effects());
                    assert_eq!(encoded.bytes().len(), width);
                    assert_eq!(
                        decode_x86_64_selected_hosted_write_byte_i32(target, encoded.bytes())
                            .map(|(_, offset)| i64::from(offset)),
                        Some(displacement)
                    );
                    // The other kernel's syscall number never replays.
                    assert!(
                        decode_x86_64_selected_hosted_write_byte_i32(other_kernel, encoded.bytes())
                            .is_none()
                    );
                    assert!(encoded.footprint().register_writes.is_empty());
                    assert_eq!(
                        encoded.footprint().encoded.stack,
                        MachineEncodedStackEffect::UnchangedV1
                    );
                    crate::selected_form_encoding::assert_every_bit_flip_rejected(
                        encoded.bytes(),
                        |changed| {
                            validate_x86_64_selected_hosted_write_byte_form(
                                target,
                                &physical,
                                kind,
                                alternative,
                                &operands,
                                displacement,
                                changed,
                            )
                            .is_err()
                        },
                    );
                    assert!(
                        validate_x86_64_selected_hosted_write_byte_form(
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
                        validate_x86_64_selected_hosted_write_byte_form(
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
                        validate_x86_64_selected_hosted_write_byte_form(
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
                        validate_x86_64_selected_hosted_write_byte_form(
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
                        encode_x86_64_selected_hosted_write_byte_form(
                            target,
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            2147483648
                        )
                        .is_err()
                    );
                    assert!(
                        encode_x86_64_selected_hosted_write_byte_form(
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
        assert!(
            decode_x86_64_selected_hosted_write_byte_i32(NativeTarget::windows_x64(), &[0; 40])
                .is_none()
        );
    }
}
