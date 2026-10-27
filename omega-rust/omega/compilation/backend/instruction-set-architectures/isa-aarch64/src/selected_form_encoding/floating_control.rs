//! Floating-control custody in a private caller-frame slot, with independent byte decoding.
use super::{
    Aarch64SelectedFormEncodingError, Aarch64SelectedFormFootprint,
    ValidatedAarch64SelectedFormEncoding,
};
use crate::machine_effects::form_alternative;
use register_model::{RegisterConstraintKey, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    LocalStorageSlotId, MachineAlternative, MachineAlternativeApplicability, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedInstructionKind,
};

/// `x9` carries the FPCR; a save past the direct `sp` reach also forms its
/// address in IP0. The constraint and the encoded effects declare the same
/// units, so both reaches stay inside one declaration.
pub(crate) fn clobbers(restore: bool) -> Vec<register_model::RegisterUnitId> {
    let physical = crate::aarch64_physical_register_model();
    let mut units = physical
        .view_named("x9")
        .expect("canonical floating-control scratch")
        .units
        .clone();
    if !restore {
        units.extend(
            physical
                .view_named("x16")
                .expect("canonical floating-control address scratch")
                .units
                .iter()
                .copied(),
        );
    }
    units.sort_unstable();
    units
}

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
        implicit_unit_clobbers: clobbers(restore),
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
        alternatives: form_alternative!(
            semantic.into(),
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 8,
                maximum_bytes: Some(crate::floating_control::MAX_FPCR_SEQUENCE_BYTES),
            },
            effects(restore)
        ),
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
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity()
        || !operands.is_empty()
        || alternative
            != (MachineAlternativeKey {
                family: semantic.into(),
                variant: 0,
            })
        || u32::try_from(displacement).is_err()
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

pub(crate) fn validate_aarch64_selected_floating_control_form(
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

/// Decode the direction and slot offset, then require the canonical bytes:
/// the access word carries the low bits and any page steps the rest.
fn decode(bytes: &[u8]) -> Option<(bool, i64)> {
    let (chunks, tail) = bytes.as_chunks::<4>();
    if !tail.is_empty() {
        return None;
    }
    let words = chunks
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect::<Vec<_>>();
    let (restore, steps, access, address) = match words.as_slice() {
        [steps @ .., access, 0xd51b_4409] => (true, steps, *access, 9),
        [0xd53b_4409, steps @ .., access] => (false, steps, *access, 16),
        _ => return None,
    };
    let pages = if steps.is_empty() {
        0
    } else {
        crate::immediate_offset::decode_address_words(steps, address, 31)?
    };
    let offset = u32::try_from(pages + u64::from((access >> 10) & 0xfff) * 8).ok()?;
    let expected = if restore {
        crate::encode_restore_fpcr_from_sp_displacement(offset)
    } else {
        crate::encode_save_fpcr_to_sp_displacement(offset)
    }
    .ok()?;
    (expected == bytes).then_some((restore, i64::from(offset)))
}

#[cfg(test)]
mod tests {
    use super::{
        effects, encode_aarch64_selected_floating_control_form,
        validate_aarch64_selected_floating_control_form,
    };
    use crate::test_support::reject_every_bit_flip;
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
            // Inside the direct reach, then page steps past 32 KiB and
            // past 16 MiB.
            for displacement in [0, 24, 32760, 32768, 65_544, 33_554_440] {
                let encoded = encode_aarch64_selected_floating_control_form(
                    &physical,
                    kind,
                    alternative,
                    &[],
                    displacement,
                )
                .unwrap();
                assert_eq!(encoded.footprint().encoded, effects(restore));
                reject_every_bit_flip(encoded.bytes(), &format!("{kind:?}"), |changed| {
                    validate_aarch64_selected_floating_control_form(
                        &physical,
                        kind,
                        alternative,
                        &[],
                        displacement,
                        &changed,
                    )
                    .is_err()
                });
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
            for displacement in [-8, 4, i64::from(u32::MAX) + 1] {
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
