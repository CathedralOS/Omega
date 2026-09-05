//! Canonical unresolved AAPCS64 ordinary scalar-call templates.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use crate::{
    AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64, aarch64_physical_register_model,
    aarch64_register_constraint_catalog,
};

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
    operand_views: [RegisterViewId; 3],
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

    pub const fn operand_views(&self) -> &[RegisterViewId; 3] {
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
        SelectedInstructionKind::CallI64 { callee } => callee,
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
    if target != NativeTarget::linux_arm64() {
        return Err(Aarch64ScalarCallTemplateError::UnsupportedTarget);
    }
    if physical.model() != &aarch64_physical_register_model() {
        return Err(Aarch64ScalarCallTemplateError::NonCanonicalPhysicalModel);
    }
    let callee = match kind {
        SelectedInstructionKind::CallI64 { callee } => callee,
        _ => return Err(Aarch64ScalarCallTemplateError::InstructionKindMismatch),
    };
    let expected_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallI64,
        variant: 0,
    };
    if alternative != expected_alternative {
        return Err(Aarch64ScalarCallTemplateError::AlternativeMismatch);
    }
    let expected_operand_views = expected_operand_views(physical);
    if operand_views != expected_operand_views {
        return Err(Aarch64ScalarCallTemplateError::OperandViewMismatch);
    }
    if effects != &expected_effects(physical) {
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

fn expected_operand_views(physical: &ValidatedPhysicalRegisterModel) -> [RegisterViewId; 3] {
    ["x0", "x1", "x0"].map(|name| {
        physical
            .model()
            .view_named(name)
            .expect("canonical AArch64 model contains scalar-call ABI view")
            .id
    })
}

fn expected_effects(physical: &ValidatedPhysicalRegisterModel) -> MachineEncodedEffects {
    let catalog = aarch64_register_constraint_catalog(physical);
    let row = catalog
        .constraints
        .iter()
        .find(|row| row.key == AARCH64_AAPCS64_CALL_I64_PAIR_TO_I64)
        .expect("canonical AArch64 catalog contains scalar-call constraint");
    MachineEncodedEffects {
        external_operand_reads: vec![0, 1],
        external_operand_writes: vec![2],
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
mod tests {
    use register_model::validate_physical_register_model;

    use super::*;

    fn inputs() -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        [RegisterViewId; 3],
        MachineEncodedEffects,
    ) {
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::CallI64 {
            callee: MachineId::new(7).unwrap(),
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::CallI64,
            variant: 0,
        };
        let operands = expected_operand_views(&physical);
        let effects = expected_effects(&physical);
        (physical, kind, alternative, operands, effects)
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
