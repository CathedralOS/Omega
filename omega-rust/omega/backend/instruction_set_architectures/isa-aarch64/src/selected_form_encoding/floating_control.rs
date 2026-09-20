//! Floating-control custody in a private caller-frame slot, with independent byte decoding.
use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
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
    let physical = crate::aarch64_physical_register_model();
    let view = |name| {
        physical
            .view_named(name)
            .expect("canonical floating-control register")
    };
    let mut uses = view("sp").units.clone();
    if !restore {
        uses.extend(view("fpcr").units.iter().copied());
    }
    uses.sort_unstable();
    MachineEncodedEffects {
        external_operand_reads: Vec::new(),
        external_operand_writes: Vec::new(),
        implicit_unit_uses: uses,
        implicit_unit_defs: if restore {
            view("fpcr").units.clone()
        } else {
            Vec::new()
        },
        implicit_unit_clobbers: view("x9").units.clone(),
        memory: if restore {
            MachineEncodedMemoryEffect::ReadFrameStorageV1 {
                stack_pointer: view("sp").id,
                byte_count: 8,
            }
        } else {
            MachineEncodedMemoryEffect::WriteFrameStorageV1 {
                stack_pointer: view("sp").id,
                byte_count: 8,
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
            size: MachineSizeKnowledge::ExactBytes(8),
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
) -> Result<bool, Aarch64SelectedFormEncodingError> {
    let semantic = match kind {
        SelectedInstructionKind::SaveFloatingControl {
            slot: LocalStorageSlotId::Boundary { .. },
        } => MachineSemanticKind::SaveFloatingControl,
        SelectedInstructionKind::RestoreFloatingControl {
            slot: LocalStorageSlotId::Boundary { .. },
        } => MachineSemanticKind::RestoreFloatingControl,
        _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
    };
    if physical.model() != &crate::aarch64_physical_register_model()
        || !operands.is_empty()
        || alternative
            != (MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            })
        || !(0..=32760).contains(&displacement)
        || displacement % 8 != 0
    {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(semantic == MachineSemanticKind::RestoreFloatingControl)
}

pub fn encode_aarch64_selected_floating_control_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let restore = request(physical, kind, alternative, operands, displacement)?;
    let offset = u32::try_from(displacement)
        .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    let bytes = if restore {
        crate::encode_restore_fpcr_from_sp_displacement(offset)
    } else {
        crate::encode_save_fpcr_to_sp_displacement(offset)
    }
    .map_err(|_| Aarch64SelectedFormEncodingError::EncodedFormMismatch)?;
    validate_aarch64_selected_floating_control_form(
        physical,
        kind,
        alternative,
        operands,
        displacement,
        &bytes,
    )
}

pub fn validate_aarch64_selected_floating_control_form(
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: &[RegisterViewId],
    displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedAarch64SelectedFormEncoding, Aarch64SelectedFormEncodingError> {
    let restore = request(physical, kind, alternative, operands, displacement)?;
    if decode(bytes) != Some((restore, displacement)) {
        return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch);
    }
    Ok(ValidatedAarch64SelectedFormEncoding {
        bytes: bytes.to_vec(),
        footprint: Aarch64SelectedFormFootprint {
            register_reads: Vec::new(),
            register_writes: Vec::new(),
            writes_nzcv: false,
            encoded: effects(restore),
        },
    })
}

fn decode(bytes: &[u8]) -> Option<(bool, i64)> {
    let words: [u8; 8] = bytes.try_into().ok()?;
    let first = u32::from_le_bytes(words[..4].try_into().ok()?);
    let second = u32::from_le_bytes(words[4..].try_into().ok()?);
    let (restore, access) = if first == 0xd53b_4409 && second & 0xffc0_03ff == 0xf900_03e9 {
        (false, second)
    } else if second == 0xd51b_4409 && first & 0xffc0_03ff == 0xf940_03e9 {
        (true, first)
    } else {
        return None;
    };
    Some((restore, i64::from((access >> 10) & 0xfff) * 8))
}

#[cfg(test)]
mod tests {
    use super::{
        effects, encode_aarch64_selected_floating_control_form,
        validate_aarch64_selected_floating_control_form,
    };
    use selected_instructions::{
        LocalStorageSlotId, MachineAlternativeKey, MachineSemanticKind, SelectedInstructionKind,
    };
    #[test]
    fn floating_controls_bind_direction_slot_displacement_and_every_encoded_bit() {
        let physical = register_model::validate_physical_register_model(
            crate::aarch64_physical_register_model(),
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
            for displacement in [0, 24, 32760] {
                let encoded = encode_aarch64_selected_floating_control_form(
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
                        validate_aarch64_selected_floating_control_form(
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
                    validate_aarch64_selected_floating_control_form(
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
                    validate_aarch64_selected_floating_control_form(
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
            for displacement in [-8, 4, 32768] {
                assert!(
                    encode_aarch64_selected_floating_control_form(
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
                encode_aarch64_selected_floating_control_form(
                    &physical,
                    wrong,
                    alternative,
                    &[],
                    0
                )
                .is_err()
            );
        }
    }
}
