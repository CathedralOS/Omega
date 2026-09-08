//! Hosted write-one-byte leaf: caller frame storage, no hidden stack adjustment.
use super::*;
use ::selected_instructions::*;
use register_model::RegisterConstraintKey;

pub(crate) fn effects() -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model();
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

pub(crate) fn declaration(constraint: RegisterConstraintKey) -> MachineEffectDeclaration {
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
            size: MachineSizeKnowledge::ExactBytes(40),
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
) -> Result<u8, X86_64SelectedFormEncodingError> {
    if physical.model() != &crate::x86_64_physical_register_model()
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
        || displacement > i32::MAX as u32
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
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let register = request(physical, kind, alternative, operands, displacement)?;
    let mut bytes = vec![
        0x40 | ((register >> 3) << 2),
        0x88,
        0x84 | ((register & 7) << 3),
        0x24,
    ];
    bytes.extend_from_slice(&displacement.to_le_bytes());
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]);
    bytes.extend_from_slice(&displacement.to_le_bytes());
    bytes.extend_from_slice(&[
        0xba, 1, 0, 0, 0, 0xb8, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f, 0x0b,
    ]);
    validate_x86_64_selected_hosted_write_byte_form(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

pub fn validate_x86_64_selected_hosted_write_byte_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: u32,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let register = request(physical, kind, alternative, operands, displacement)?;
    if decode(bytes) != Some((register, displacement)) {
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

fn decode(bytes: &[u8]) -> Option<(u8, u32)> {
    if bytes.len() != 40 {
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
        || bytes[21..]
            != [
                0xba, 1, 0, 0, 0, 0xb8, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f,
                0x0b,
            ]
    {
        return None;
    }
    Some((decoded_register, stored_offset))
}

/// Independently decode the complete returning write leaf for object custody replay.
pub fn decode_x86_64_selected_hosted_write_byte_i32(
    bytes: &[u8],
) -> Option<(calling_conventions::MachineRegister, u32)> {
    let (register, displacement) = decode(bytes)?;
    use calling_conventions::MachineRegister::*;
    let registers = [
        X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
        X86R11, X86R12, X86R13, X86R14, X86R15,
    ];
    Some((registers[usize::from(register)], displacement))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        for name in ["rax", "rbp", "rsi", "rdx", "r11", "r15"] {
            let operands = [physical.model().view_named(name).unwrap().id];
            for displacement in [0, 31, 2147483647] {
                let encoded = encode_x86_64_selected_hosted_write_byte_form(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.footprint().encoded, effects());
                assert!(encoded.footprint().register_writes.is_empty());
                assert_eq!(
                    encoded.footprint().encoded.stack,
                    MachineEncodedStackEffect::UnchangedV1
                );
                for bit in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate_x86_64_selected_hosted_write_byte_form(
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
                    validate_x86_64_selected_hosted_write_byte_form(
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
