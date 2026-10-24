//! Fixed-frame addresses and stores share one home locator. Absolute alignment
//! rounds only the reserved base, before member offsets. Store64 always defines
//! its allocated early-clobber address scratch, including unaligned homes;
//! neither frame placement nor encoding may invent a register after allocation.
//! The address mask can change flags, so both forms conservatively declare the
//! RFLAGS clobber in the target catalog before instruction scheduling.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};

use super::super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint,
};

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
) -> Result<(RegisterViewId, Option<RegisterViewId>), X86_64SelectedFormEncodingError> {
    let (family, store) = match kind {
        SelectedInstructionKind::Store64 { .. } => (MachineAlternativeFamily::Store64, true),
        SelectedInstructionKind::FrameAddress { .. } => {
            (MachineAlternativeFamily::FrameAddress, false)
        }
        _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
    };
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
        || alternative != (MachineAlternativeKey { family, variant: 0 })
    {
        return Err(X86_64SelectedFormEncodingError::AlternativeMismatch);
    }
    super::super::request_validation::resolve_registers(physical, operands)?;
    match operands {
        [value, scratch] if store && value != scratch => Ok((*scratch, Some(*value))),
        [address] if !store => Ok((*address, None)),
        _ => Err(X86_64SelectedFormEncodingError::OperandCountMismatch),
    }
}

pub fn encode(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    alignment_bytes: u16,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (address, value) = request(physical, kind, alternative, operands)?;
    let displacement = i32::try_from(displacement)
        .map_err(|_| X86_64SelectedFormEncodingError::ImmediateOutsideU12)?;
    let member = match kind {
        SelectedInstructionKind::Store64 { byte_offset, .. }
        | SelectedInstructionKind::FrameAddress { byte_offset, .. } => byte_offset,
        _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
    };
    let mut bytes = if alignment_bytes == 1 {
        crate::frame_protocol::encode_frame_address_signed(physical, address, displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?
    } else {
        let offset = u32::try_from(displacement)
            .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
        crate::encode_x86_64_aligned_frame_address(
            physical,
            address,
            offset,
            alignment_bytes,
            member,
        )
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?
    };
    if let Some(value) = value {
        bytes.extend_from_slice(
            &crate::encode_x86_64_pointer_memory(physical, value, address, 0, 8, false)
                .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?,
        );
    }
    validate(
        physical,
        kind,
        alternative,
        operands,
        i64::from(displacement),
        alignment_bytes,
        &bytes,
    )
}

pub fn validate(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    alignment_bytes: u16,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let (address, value) = request(physical, kind, alternative, operands)?;
    let displacement = i32::try_from(displacement)
        .map_err(|_| X86_64SelectedFormEncodingError::ImmediateOutsideU12)?;
    let member = match kind {
        SelectedInstructionKind::Store64 { byte_offset, .. }
        | SelectedInstructionKind::FrameAddress { byte_offset, .. } => byte_offset,
        _ => return Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
    };
    let address_bytes = if let Some(value) = value {
        // The pointer-store decoder establishes the exact register pair and
        // zero member displacement. Its frame address is decoded separately.
        crate::frame_protocol::decode_pointer_store_suffix(physical, value, address, bytes)
            .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?
    } else {
        bytes
    };
    if !crate::frame_protocol::validate_frame_address(
        physical,
        address,
        displacement,
        alignment_bytes,
        member,
        address_bytes,
    ) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    let store = value.is_some();
    let catalog = crate::x86_64_register_constraint_catalog_for(physical);
    let key = if store {
        crate::X86_64_STORE64
    } else {
        crate::X86_64_FRAME_ADDRESS
    };
    let constraint = catalog
        .constraint(key)
        .ok_or(X86_64SelectedFormEncodingError::AlternativeMismatch)?;
    let stack_pointer = physical
        .model()
        .view_named("rsp")
        .ok_or(X86_64SelectedFormEncodingError::EncodedFormMismatch)?
        .id;
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: value.into_iter().collect(),
            register_writes: vec![address],
            writes_rflags: alignment_bytes != 1,
            encoded: MachineEncodedEffects {
                external_operand_reads: if store { vec![0] } else { Vec::new() },
                external_operand_writes: vec![u16::from(store)],
                implicit_unit_uses: constraint.implicit_uses.clone(),
                implicit_unit_defs: constraint.implicit_defs.clone(),
                implicit_unit_clobbers: constraint.clobbers.clone(),
                memory: if store {
                    MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                        stack_pointer,
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
    use super::super::replay_battery;
    use super::*;
    use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};
    use semantic_vocabulary::{OperationId, PlaceId};

    #[test]
    fn aligned_frame_forms_decode_members_and_declared_clobbers() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let value = physical.model().view_named("r9").unwrap().id;
        let address = physical.model().view_named("r12").unwrap().id;
        let slot = FrameStorageSlotId::Local(LocalStorageSlotId::Structural {
            operation: OperationId::new(7).unwrap(),
            place: PlaceId::new(1).unwrap(),
        });
        for (kind, family, operands) in [
            (
                SelectedInstructionKind::FrameAddress {
                    slot,
                    byte_offset: 24,
                },
                MachineAlternativeFamily::FrameAddress,
                vec![address],
            ),
            (
                SelectedInstructionKind::Store64 {
                    slot,
                    byte_offset: 24,
                },
                MachineAlternativeFamily::Store64,
                vec![value, address],
            ),
        ] {
            let alternative = MachineAlternativeKey { family, variant: 0 };
            let encoded = encode(&physical, kind, alternative, &operands, 48, 64).unwrap();
            assert!(encoded.footprint.writes_rflags);
            assert_eq!(encoded.footprint.register_writes, vec![address]);
            assert_eq!(
                encoded.footprint.encoded.implicit_unit_clobbers,
                physical.model().view_named("rflags").unwrap().units
            );
            // The address mask is followed by the member LEA, never vice versa.
            assert_eq!(
                &encoded.bytes[..15],
                &[
                    0x4c, 0x8d, 0xa4, 0x24, 111, 0, 0, 0, 0x49, 0x81, 0xe4, 0xc0, 0xff, 0xff, 0xff
                ]
            );
            assert_eq!(
                &encoded.bytes[15..23],
                &[0x4d, 0x8d, 0xa4, 0x24, 24, 0, 0, 0]
            );
            replay_battery::replay_rejects_byte_mutations(&encoded.bytes, |bytes| {
                validate(&physical, kind, alternative, &operands, 48, 64, bytes)
            });
            assert!(
                validate(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    64,
                    64,
                    &encoded.bytes
                )
                .is_err()
            );
            assert!(
                validate(
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    48,
                    32,
                    &encoded.bytes
                )
                .is_err()
            );
            assert!(encode(&physical, kind, alternative, &operands, 49, 64).is_err());
            assert!(encode(&physical, kind, alternative, &operands, 48, 48).is_err());
        }
        assert!(
            encode(
                &physical,
                SelectedInstructionKind::Store64 {
                    slot,
                    byte_offset: 0
                },
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::Store64,
                    variant: 0
                },
                &[value, value],
                48,
                32
            )
            .is_err()
        );
    }
}
