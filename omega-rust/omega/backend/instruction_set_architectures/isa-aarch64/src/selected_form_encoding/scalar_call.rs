//! Canonical unresolved AArch64 ordinary register scalar-call templates.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use crate::machine_effects::{Aarch64SelectedAbi, aarch64_selected_abi};
use crate::{aarch64_aapcs64_register_call_keys, aarch64_register_constraint_catalog_for};

pub const AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT: usize = 4;
pub const AARCH64_SCALAR_CALL_OPCODE_OFFSET: u16 = 0;
pub const AARCH64_SCALAR_CALL_PATCH_OFFSET: u16 = 0;
pub const AARCH64_SCALAR_CALL_REFERENCE_OFFSET: u16 = 0;
pub const AARCH64_SCALAR_CALL_PATCH_WIDTH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64ScalarCallFixupKind {
    SignedImmediate26WordsFromInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64ScalarCallFixupState {
    UnresolvedZeroImmediateV1,
}

/// Target-owned unresolved control fixup for one ordinary scalar call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64ScalarCallFixup {
    pub kind: Aarch64ScalarCallFixupKind,
    pub state: Aarch64ScalarCallFixupState,
    pub callee: MachineId,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
}

/// Exact selected inputs and canonical bytes for one unresolved scalar call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SelectedScalarCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: [u8; AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT],
    fixup: Aarch64ScalarCallFixup,
}

impl ValidatedAarch64SelectedScalarCallTemplate {
    pub const fn kind(&self) -> SelectedInstructionKind {
        self.kind
    }

    pub const fn alternative(&self) -> MachineAlternativeKey {
        self.alternative
    }

    pub fn operand_views(&self) -> &[RegisterViewId] {
        &self.operand_views
    }

    pub const fn effects(&self) -> &MachineEncodedEffects {
        &self.effects
    }

    pub const fn bytes(&self) -> &[u8; AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] {
        &self.bytes
    }

    pub const fn fixup(&self) -> Aarch64ScalarCallFixup {
        self.fixup
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64ScalarCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for Aarch64ScalarCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid AArch64 scalar-call template: {self:?}")
    }
}

impl std::error::Error for Aarch64ScalarCallTemplateError {}

/// Produce the canonical `BL #0` word with an unresolved internal-call fixup.
pub fn encode_aarch64_selected_scalar_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
) -> Result<ValidatedAarch64SelectedScalarCallTemplate, Aarch64ScalarCallTemplateError> {
    let callee = match kind {
        SelectedInstructionKind::CallScalar { callee }
        | SelectedInstructionKind::CallAggregate { callee }
        | SelectedInstructionKind::CallUnit { callee } => callee,
        _ => return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch),
    };
    let bytes = 0x9400_0000_u32.to_le_bytes();
    let fixup = canonical_fixup(callee);
    validate_aarch64_selected_scalar_call_template(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        effects,
        &bytes,
        fixup,
    )
}

/// Independently validate every selected input, byte, and unresolved fixup.
pub fn validate_aarch64_selected_scalar_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: Aarch64ScalarCallFixup,
) -> Result<ValidatedAarch64SelectedScalarCallTemplate, Aarch64ScalarCallTemplateError> {
    // The scalar-call encoding matrix is the declared (architecture,
    // object-format) pair matrix in `machine_effects`: (Aarch64, Elf) selects
    // AAPCS64 rosters and (Aarch64, Mach-O) selects the Darwin variant.
    // Undeclared pairs — (Aarch64, Coff) and every other architecture — fail
    // closed here rather than falling through an else arm into one family.
    let abi = aarch64_selected_abi(target)
        .map_err(|_| Aarch64ScalarCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64ScalarCallTemplateError::NonCanonicalPhysicalModel);
    }
    let callee = match kind {
        SelectedInstructionKind::CallScalar { callee }
        | SelectedInstructionKind::CallAggregate { callee }
        | SelectedInstructionKind::CallUnit { callee } => callee,
        _ => return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch),
    };
    let unit = matches!(kind, SelectedInstructionKind::CallUnit { .. });
    let expected_alternative = MachineAlternativeKey {
        family: if unit {
            MachineAlternativeFamily::CallUnit
        } else if matches!(kind, SelectedInstructionKind::CallAggregate { .. }) {
            MachineAlternativeFamily::CallAggregate
        } else {
            MachineAlternativeFamily::CallScalar
        },
        variant: 0,
    };
    if alternative != expected_alternative {
        return Err(Aarch64ScalarCallTemplateError::AlternativeMismatch);
    }
    let aggregate = matches!(kind, SelectedInstructionKind::CallAggregate { .. });
    let floating_scalar = !unit
        && !aggregate
        && operand_views.iter().any(|id| {
            physical
                .model()
                .views
                .iter()
                .any(|view| view.id == *id && view.name.starts_with("d"))
        });
    let (expected_operand_views, expected) = if unit || aggregate || floating_scalar {
        let darwin = abi == Aarch64SelectedAbi::Darwin;
        let keys = if aggregate {
            crate::aarch64_register_aggregate_call_keys(darwin)
                .into_iter()
                .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                .collect::<Vec<_>>()
        } else if floating_scalar {
            crate::aarch64_float_scalar_call_keys(darwin)
        } else {
            match abi {
                Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_register_unit_call_keys()
                    .into_iter()
                    .chain(crate::aarch64_aapcs64_mixed_unit_call_keys())
                    .collect::<Vec<_>>(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_unit_call_keys()
                    .into_iter()
                    .chain(crate::aarch64_darwin_mixed_unit_call_keys())
                    .collect::<Vec<_>>(),
            }
        };
        let catalog = aarch64_register_constraint_catalog_for(physical);
        let row = catalog
            .constraints
            .iter()
            .find(|row| {
                keys.contains(&row.key)
                    && row.operands.len() == operand_views.len()
                    && row
                        .operands
                        .iter()
                        .zip(operand_views)
                        .all(|(operand, view)| operand.fixed_view == Some(*view))
            })
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
        let mut expected = expected_effects(abi, physical, 0);
        expected.external_operand_reads = row
            .operands
            .iter()
            .filter(|operand| operand.access == register_model::RegisterOperandAccess::Use)
            .map(|operand| operand.operand)
            .collect();
        expected.external_operand_writes = row
            .operands
            .iter()
            .filter(|operand| operand.access == register_model::RegisterOperandAccess::Def)
            .map(|operand| operand.operand)
            .collect();
        expected.implicit_unit_uses = row.implicit_uses.clone();
        expected.implicit_unit_defs = row.implicit_defs.clone();
        expected.implicit_unit_clobbers = row.clobbers.clone();
        (operand_views.to_vec(), expected)
    } else {
        let arity = operand_views
            .len()
            .checked_sub(1)
            .filter(|arity| *arity <= 8)
            .ok_or(Aarch64ScalarCallTemplateError::OperandViewMismatch)?;
        let expected_operand_views = expected_operand_views(physical, arity);
        if operand_views != expected_operand_views {
            return Err(Aarch64ScalarCallTemplateError::OperandViewMismatch);
        }
        (
            expected_operand_views,
            expected_effects(abi, physical, arity),
        )
    };
    if effects != &expected {
        return Err(Aarch64ScalarCallTemplateError::EffectMismatch);
    }
    let bytes: [u8; AARCH64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] = bytes
        .try_into()
        .map_err(|_| Aarch64ScalarCallTemplateError::MalformedTemplate)?;
    if u32::from_le_bytes(bytes) != 0x9400_0000 {
        return Err(Aarch64ScalarCallTemplateError::MalformedTemplate);
    }
    if fixup != canonical_fixup(callee) {
        return Err(Aarch64ScalarCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedAarch64SelectedScalarCallTemplate {
        kind,
        alternative,
        operand_views: expected_operand_views,
        effects: effects.clone(),
        bytes,
        fixup,
    })
}

fn canonical_fixup(callee: MachineId) -> Aarch64ScalarCallFixup {
    Aarch64ScalarCallFixup {
        kind: Aarch64ScalarCallFixupKind::SignedImmediate26WordsFromInstructionToInternalMachineV1,
        state: Aarch64ScalarCallFixupState::UnresolvedZeroImmediateV1,
        callee,
        opcode_byte_offset: AARCH64_SCALAR_CALL_OPCODE_OFFSET,
        patch_byte_offset: AARCH64_SCALAR_CALL_PATCH_OFFSET,
        reference_byte_offset: AARCH64_SCALAR_CALL_REFERENCE_OFFSET,
        patch_byte_width: AARCH64_SCALAR_CALL_PATCH_WIDTH,
    }
}

fn expected_operand_views(
    physical: &ValidatedPhysicalRegisterModel,
    arity: usize,
) -> Vec<RegisterViewId> {
    ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"]
        .into_iter()
        .take(arity)
        .chain(["x0"])
        .map(|name| {
            physical
                .model()
                .view_named(name)
                .expect("canonical ABI register")
                .id
        })
        .collect()
}

fn expected_effects(
    abi: Aarch64SelectedAbi,
    physical: &ValidatedPhysicalRegisterModel,
    arity: usize,
) -> MachineEncodedEffects {
    let catalog = aarch64_register_constraint_catalog_for(physical);
    let row = catalog
        .constraints
        .iter()
        .find(|row| {
            row.key
                == match abi {
                    Aarch64SelectedAbi::Aapcs64 => aarch64_aapcs64_register_call_keys()[arity],
                    Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_call_keys()[arity],
                }
        })
        .expect("canonical AArch64 catalog contains scalar-call constraint");
    MachineEncodedEffects {
        external_operand_reads: (0..arity as u16).collect(),
        external_operand_writes: vec![arity as u16],
        implicit_unit_uses: row.implicit_uses.clone(),
        implicit_unit_defs: row.implicit_defs.clone(),
        implicit_unit_clobbers: row.clobbers.clone(),
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::DirectRelativeCallV1,
    }
}

#[cfg(test)]
mod mixed_calls;
#[cfg(test)]
mod target_abis;
#[cfg(test)]
mod unit_calls;

#[cfg(test)]
mod tests {
    use super::{
        Aarch64ScalarCallFixup, Aarch64ScalarCallTemplateError, Aarch64SelectedAbi,
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineId,
        NativeTarget, RegisterViewId, SelectedInstructionKind, ValidatedPhysicalRegisterModel,
        aarch64_aapcs64_register_call_keys, aarch64_selected_abi, canonical_fixup,
        encode_aarch64_selected_scalar_call_template, expected_effects, expected_operand_views,
        validate_aarch64_selected_scalar_call_template,
    };
    use crate::aarch64_physical_register_model;
    use register_model::validate_physical_register_model;

    fn inputs() -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        Vec<RegisterViewId>,
        MachineEncodedEffects,
    ) {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::CallScalar {
            callee: MachineId::new(7).unwrap(),
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::CallScalar,
            variant: 0,
        };
        let operands = expected_operand_views(&physical, 2);
        let effects = expected_effects(Aarch64SelectedAbi::Aapcs64, &physical, 2);
        (physical, kind, alternative, operands, effects)
    }

    #[test]
    fn every_register_arity_has_exact_operands_and_effects() {
        let (physical, kind, alternative, _, _) = inputs();
        let catalog = crate::aarch64_register_constraint_catalog(&physical);
        for (arity, key) in aarch64_aapcs64_register_call_keys().into_iter().enumerate() {
            let row = catalog
                .constraints
                .iter()
                .find(|row| row.key == key)
                .unwrap();
            let operands = expected_operand_views(&physical, arity);
            assert_eq!(row.operands.len(), arity + 1);
            assert_eq!(
                row.operands
                    .iter()
                    .map(|operand| operand.fixed_view.unwrap())
                    .collect::<Vec<_>>(),
                operands
            );
            let effects = expected_effects(Aarch64SelectedAbi::Aapcs64, &physical, arity);
            assert_eq!(
                effects.external_operand_reads,
                (0..arity as u16).collect::<Vec<_>>()
            );
            assert_eq!(effects.external_operand_writes, vec![arity as u16]);
            let template = encode_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
            )
            .unwrap();
            assert_eq!(template.operand_views(), operands.as_slice());
            let mut wrong_effects = effects.clone();
            wrong_effects.external_operand_writes = vec![(arity + 1) as u16];
            assert!(
                encode_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &wrong_effects,
                )
                .is_err()
            );
            let mut wrong_views = operands.clone();
            wrong_views[arity] = physical.model().view_named("sp").unwrap().id;
            assert!(
                encode_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &wrong_views,
                    &effects,
                )
                .is_err()
            );
        }
        let oversized = vec![physical.model().view_named("x0").unwrap().id; 10];
        assert_eq!(
            encode_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &oversized,
                &expected_effects(Aarch64SelectedAbi::Aapcs64, &physical, 0),
            ),
            Err(Aarch64ScalarCallTemplateError::OperandViewMismatch)
        );
    }

    #[test]
    fn scalar_call_abi_matrix_resolves_declared_pairs_and_fails_closed() {
        assert_eq!(
            aarch64_selected_abi(NativeTarget::linux_arm64()),
            Ok(Aarch64SelectedAbi::Aapcs64)
        );
        assert_eq!(
            aarch64_selected_abi(NativeTarget::macos_arm64()),
            Ok(Aarch64SelectedAbi::Darwin)
        );
        for undeclared in [
            NativeTarget {
                object_format: target::ObjectFormat::Coff,
                ..NativeTarget::linux_arm64()
            },
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
        ] {
            assert_eq!(
                aarch64_selected_abi(undeclared),
                Err(
                    crate::machine_effects::Aarch64MachineEffectCatalogValidationError::UnsupportedTargetAbi,
                )
            );
        }
    }

    #[test]
    fn scalar_call_template_has_exact_bytes_and_fixup() {
        let (physical, kind, alternative, operands, effects) = inputs();
        let template = encode_aarch64_selected_scalar_call_template(
            NativeTarget::linux_arm64(),
            &physical,
            kind,
            alternative,
            &operands,
            &effects,
        )
        .unwrap();
        assert_eq!(template.bytes(), &0x9400_0000_u32.to_le_bytes());
        assert_eq!(template.kind(), kind);
        assert_eq!(template.alternative(), alternative);
        assert_eq!(template.operand_views(), &operands);
        assert_eq!(template.effects(), &effects);
        assert_eq!(
            template.fixup(),
            canonical_fixup(MachineId::new(7).unwrap())
        );
    }

    #[test]
    fn scalar_call_validation_rejects_malformed_selected_inputs() {
        let (physical, kind, alternative, operands, effects) = inputs();
        let fixup = canonical_fixup(MachineId::new(7).unwrap());
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                fixup,
            ),
            Err(Aarch64ScalarCallTemplateError::UnsupportedTarget)
        );
        // Undeclared (architecture, object-format) pairs fail closed through
        // the same matrix: an AArch64 COFF target does not inherit either
        // declared row.
        let undeclared_coff = NativeTarget {
            object_format: target::ObjectFormat::Coff,
            ..NativeTarget::linux_arm64()
        };
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                undeclared_coff,
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                fixup,
            ),
            Err(Aarch64ScalarCallTemplateError::UnsupportedTarget)
        );
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                SelectedInstructionKind::ReturnUnit,
                alternative,
                &operands,
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                fixup,
            ),
            Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch)
        );
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                MachineAlternativeKey {
                    variant: 1,
                    ..alternative
                },
                &operands,
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                fixup,
            ),
            Err(Aarch64ScalarCallTemplateError::AlternativeMismatch)
        );
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands[..2],
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                fixup,
            ),
            Err(Aarch64ScalarCallTemplateError::OperandViewMismatch)
        );
        let mut malformed_effects = effects.clone();
        malformed_effects.external_operand_reads.clear();
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &malformed_effects,
                &0x9400_0000_u32.to_le_bytes(),
                fixup,
            ),
            Err(Aarch64ScalarCallTemplateError::EffectMismatch)
        );
    }

    #[test]
    fn scalar_call_validation_rejects_nonzero_or_trailing_bytes_and_bad_fixup() {
        let (physical, kind, alternative, operands, effects) = inputs();
        let fixup = canonical_fixup(MachineId::new(7).unwrap());
        for malformed in [
            &0x1400_0000_u32.to_le_bytes()[..],
            &0x9400_0001_u32.to_le_bytes()[..],
            &[0, 0, 0x94][..],
            &[0, 0, 0, 0x94, 0][..],
        ] {
            assert_eq!(
                validate_aarch64_selected_scalar_call_template(
                    NativeTarget::linux_arm64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    malformed,
                    fixup,
                ),
                Err(Aarch64ScalarCallTemplateError::MalformedTemplate)
            );
        }
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                Aarch64ScalarCallFixup {
                    callee: MachineId::new(8).unwrap(),
                    ..fixup
                },
            ),
            Err(Aarch64ScalarCallTemplateError::FixupMismatch)
        );
        assert_eq!(
            validate_aarch64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &0x9400_0000_u32.to_le_bytes(),
                Aarch64ScalarCallFixup {
                    reference_byte_offset: 4,
                    ..fixup
                },
            ),
            Err(Aarch64ScalarCallTemplateError::FixupMismatch)
        );
    }
}
