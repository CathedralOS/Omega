//! Floating-control custody in a private caller-frame slot, with independent byte decoding.
use super::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    X86_64SelectedFormFootprint,
};
use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    LocalStorageSlotId, MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedInstructionKind,
};

pub(crate) fn effects(restore: bool) -> MachineEncodedEffects {
    let physical = crate::x86_64_physical_register_model();
    let view = |name| {
        physical
            .view_named(name)
            .expect("canonical floating-control register")
    };
    let mut uses = view("rsp").units.clone();
    if !restore {
        uses.extend(view("mxcsr").units.iter().copied());
    }
    uses.sort_unstable();
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: if restore {
            view("mxcsr").units.clone()
        } else {
            Vec::new()
        },
        implicit_unit_clobbers: Vec::new(),
        memory: if restore {
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                stack_pointer: view("rsp").id,
                byte_count: 4,
            }
        } else {
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer: view("rsp").id,
                byte_count: 4,
            }
        },
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::FallThroughV1,
    }
}

pub(crate) fn declaration(
    semantic: MachineSemanticKind,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let restore = semantic == MachineSemanticKind::RestoreFloatingControl;
    MachineEffectDeclaration {
        semantic,
        constraint,
        memory: if restore {
            MachineMemoryEffect::ReadFrameStorageV1
        } else {
            MachineMemoryEffect::WriteFrameStorageV1
        },
        trap: MachineTrapBehavior::MayArchitecturalFaultV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes: Some(8),
            },
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(restore),
        }],
    }
}

fn request(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<bool, X86_64SelectedFormEncodingError> {
    let semantic = match kind {
        SelectedInstructionKind::SaveFloatingControl {
            slot: LocalStorageSlotId::Boundary { .. },
        } => MachineSemanticKind::SaveFloatingControl,
        SelectedInstructionKind::RestoreFloatingControl {
            slot: LocalStorageSlotId::Boundary { .. },
        } => MachineSemanticKind::RestoreFloatingControl,
        _ => return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch),
    };
    if physical.model() != &crate::x86_64_physical_register_model()
        || !operands.is_empty()
        || alternative
            != (MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            })
        || !(0..=i64::from(i32::MAX)).contains(&displacement)
        || displacement % 8 != 0
    {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(semantic == MachineSemanticKind::RestoreFloatingControl)
}

pub fn encode_x86_64_selected_floating_control_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let restore = request(physical, kind, alternative, operands, displacement)?;
    let offset = u32::try_from(displacement)
        .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    let bytes = if restore {
        crate::encode_ldmxcsr_rsp_displacement(offset)
    } else {
        crate::encode_stmxcsr_rsp_displacement(offset)
    }
    .map_err(|_| X86_64SelectedFormEncodingError::EncodedFormMismatch)?;
    validate_x86_64_selected_floating_control_form(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

pub fn validate_x86_64_selected_floating_control_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError> {
    let restore = request(physical, kind, alternative, operands, displacement)?;
    if decode(bytes) != Some((restore, displacement)) {
        return Err(X86_64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedX86_64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: X86_64SelectedFormFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            writes_rflags: false,
            encoded: effects(restore),
        },
    })
}

fn decode(bytes: &[u8]) -> Option<(bool, i64)> {
    let [0x0f, 0xae, mode, 0x24, tail @ ..] = bytes else {
        return None;
    };
    let restore = match mode & 0x3f {
        0x14 => true,
        0x1c => false,
        _ => return None,
    };
    let displacement = match (mode >> 6, tail) {
        (0, []) => 0,
        (1, [offset]) if (1..=127).contains(offset) => i64::from(*offset),
        (2, encoded_displacement) if encoded_displacement.len() == 4 => {
            let offset = i32::from_le_bytes(encoded_displacement.try_into().ok()?);
            if offset < 128 {
                return None;
            }
            i64::from(offset)
        }
        _ => return None,
    };
    Some((restore, displacement))
}

#[cfg(test)]
mod tests {
    use super::{
        effects, encode_x86_64_selected_floating_control_form,
        validate_x86_64_selected_floating_control_form,
    };
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeKey, MachineSemanticKind, SelectedInstructionKind,
    };
    #[test]
    fn floating_controls_bind_direction_slot_displacement_and_every_encoded_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::x86_64_physical_register_model(),
        )
        .unwrap();
        let slot = LocalStorageSlotId::Boundary {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
        };
        for restore in [false, true] {
            let kind = if restore {
                SelectedInstructionKind::RestoreFloatingControl { slot }
            } else {
                SelectedInstructionKind::SaveFloatingControl { slot }
            };
            let semantic = if restore {
                MachineSemanticKind::RestoreFloatingControl
            } else {
                MachineSemanticKind::SaveFloatingControl
            };
            let alternative = MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            };
            for displacement in [0, 8, 120, 128, 4096, 2147483640] {
                let encoded = encode_x86_64_selected_floating_control_form(
                    &physical,
                    kind,
                    alternative,
                    &[],
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.footprint().encoded, effects(restore));
                for bit in 0..encoded.bytes().len() * 8 {
                    let mut changed = encoded.bytes().to_vec();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        validate_x86_64_selected_floating_control_form(
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
                }
                assert!(
                    validate_x86_64_selected_floating_control_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement + 8,
                        encoded.bytes()
                    )
                    .is_err()
                );
                let mut extra = encoded.bytes().to_vec();
                extra.push(0);
                assert!(
                    validate_x86_64_selected_floating_control_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement,
                        &extra
                    )
                    .is_err()
                );
            }
            for displacement in [-8, 4, 2147483648] {
                assert!(
                    encode_x86_64_selected_floating_control_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement
                    )
                    .is_err()
                );
            }
            let wrong = SelectedInstructionKind::SaveFloatingControl {
                slot: LocalStorageSlotId::Structural {
                    operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                    place: semantic_vocabulary::PlaceId::new(1).unwrap(),
                },
            };
            assert!(
                encode_x86_64_selected_floating_control_form(&physical, wrong, alternative, &[], 0)
                    .is_err()
            );
        }
    }
}
