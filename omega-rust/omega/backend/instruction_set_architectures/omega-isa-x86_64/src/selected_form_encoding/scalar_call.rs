//! Canonical unresolved System V AMD64 ordinary scalar-call templates.

use omega_register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::{
    X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64, x86_64_physical_register_model,
    x86_64_register_constraint_catalog,
};

pub const X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT: usize = 5;
pub const X86_64_SCALAR_CALL_OPCODE_OFFSET: u16 = 0;
pub const X86_64_SCALAR_CALL_PATCH_OFFSET: u16 = 1;
pub const X86_64_SCALAR_CALL_REFERENCE_OFFSET: u16 = 5;
pub const X86_64_SCALAR_CALL_PATCH_WIDTH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64ScalarCallFixupKind {
    Relative32FromNextInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64ScalarCallFixupState {
    UnresolvedZeroFieldV1,
}

/// Target-owned unresolved control fixup for one ordinary scalar call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64ScalarCallFixup {
    pub kind: X86_64ScalarCallFixupKind,
    pub state: X86_64ScalarCallFixupState,
    pub callee: MachineId,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
}

/// Exact selected inputs and canonical bytes for one unresolved scalar call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedScalarCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: [RegisterViewId; 3],
    effects: MachineEncodedEffects,
    bytes: [u8; X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT],
    fixup: X86_64ScalarCallFixup,
}

impl ValidatedX86_64SelectedScalarCallTemplate {
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

    pub const fn bytes(&self) -> &[u8; X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] {
        &self.bytes
    }

    pub const fn fixup(&self) -> X86_64ScalarCallFixup {
        self.fixup
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64ScalarCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for X86_64ScalarCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid x86-64 scalar-call template: {self:?}")
    }
}

impl std::error::Error for X86_64ScalarCallTemplateError {}

/// Produce `E8 + zero rel32` with an explicit unresolved internal-call fixup.
pub fn encode_x86_64_selected_scalar_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
) -> Result<ValidatedX86_64SelectedScalarCallTemplate, X86_64ScalarCallTemplateError> {
    let callee = match kind {
        SelectedInstructionKind::CallI64 { callee } => callee,
        _ => return Err(X86_64ScalarCallTemplateError::InstructionKindMismatch),
    };
    let bytes = [0xe8, 0, 0, 0, 0];
    let fixup = canonical_fixup(callee);
    validate_x86_64_selected_scalar_call_template(
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
pub fn validate_x86_64_selected_scalar_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: X86_64ScalarCallFixup,
) -> Result<ValidatedX86_64SelectedScalarCallTemplate, X86_64ScalarCallTemplateError> {
    if target != NativeTarget::linux_x64() {
        return Err(X86_64ScalarCallTemplateError::UnsupportedTarget);
    }
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel);
    }
    let callee = match kind {
        SelectedInstructionKind::CallI64 { callee } => callee,
        _ => return Err(X86_64ScalarCallTemplateError::InstructionKindMismatch),
    };
    let expected_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::CallI64,
        variant: 0,
    };
    if alternative != expected_alternative {
        return Err(X86_64ScalarCallTemplateError::AlternativeMismatch);
    }
    let expected_operand_views = expected_operand_views(physical);
    if operand_views != expected_operand_views {
        return Err(X86_64ScalarCallTemplateError::OperandViewMismatch);
    }
    if effects != &expected_effects(physical) {
        return Err(X86_64ScalarCallTemplateError::EffectMismatch);
    }
    let bytes: [u8; X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT] = bytes
        .try_into()
        .map_err(|_| X86_64ScalarCallTemplateError::MalformedTemplate)?;
    if bytes != [0xe8, 0, 0, 0, 0] {
        return Err(X86_64ScalarCallTemplateError::MalformedTemplate);
    }
    if fixup != canonical_fixup(callee) {
        return Err(X86_64ScalarCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedX86_64SelectedScalarCallTemplate {
        kind,
        alternative,
        operand_views: expected_operand_views,
        effects: effects.clone(),
        bytes,
        fixup,
    })
}

fn canonical_fixup(callee: MachineId) -> X86_64ScalarCallFixup {
    X86_64ScalarCallFixup {
        kind: X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1,
        state: X86_64ScalarCallFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_byte_offset: X86_64_SCALAR_CALL_OPCODE_OFFSET,
        patch_byte_offset: X86_64_SCALAR_CALL_PATCH_OFFSET,
        reference_byte_offset: X86_64_SCALAR_CALL_REFERENCE_OFFSET,
        patch_byte_width: X86_64_SCALAR_CALL_PATCH_WIDTH,
    }
}

fn expected_operand_views(physical: &ValidatedPhysicalRegisterModel) -> [RegisterViewId; 3] {
    ["rdi", "rsi", "rax"].map(|name| {
        physical
            .model()
            .view_named(name)
            .expect("canonical x86-64 model contains scalar-call ABI view")
            .id
    })
}

fn expected_effects(physical: &ValidatedPhysicalRegisterModel) -> MachineEncodedEffects {
    let catalog = x86_64_register_constraint_catalog(physical);
    let row = catalog
        .constraints
        .iter()
        .find(|row| row.key == X86_64_SYSTEM_V_CALL_I64_PAIR_TO_I64)
        .expect("canonical x86-64 catalog contains scalar-call constraint");
    let stack_pointer = physical
        .model()
        .view_named("rsp")
        .expect("canonical x86-64 model contains rsp")
        .id;
    MachineEncodedEffects {
        external_operand_reads: vec![0, 1],
        external_operand_writes: vec![2],
        implicit_unit_uses: row.implicit_uses.clone(),
        implicit_unit_defs: row.implicit_defs.clone(),
        implicit_unit_clobbers: row.clobbers.clone(),
        memory: MachineEncodedMemoryEffect::WriteReturnAddressBelowStackPointerV1 {
            stack_pointer,
            byte_count: 8,
        },
        stack: MachineEncodedStackEffect::CallReturnAddressLifecycleV1 {
            stack_pointer,
            return_address_byte_count: 8,
        },
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::DirectRelativeCallV1,
    }
}

#[cfg(test)]
mod tests {
    use omega_register_model::validate_physical_register_model;

    use super::*;

    fn inputs() -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        [RegisterViewId; 3],
        MachineEncodedEffects,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
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
        let template = encode_x86_64_selected_scalar_call_template(
            NativeTarget::linux_x64(),
            &physical,
            kind,
            alternative,
            &operands,
            &effects,
        )
        .unwrap();
        assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
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
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &[0xe8, 0, 0, 0, 0],
                fixup,
            ),
            Err(X86_64ScalarCallTemplateError::UnsupportedTarget)
        );
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                SelectedInstructionKind::ReturnUnit,
                alternative,
                &operands,
                &effects,
                &[0xe8, 0, 0, 0, 0],
                fixup,
            ),
            Err(X86_64ScalarCallTemplateError::InstructionKindMismatch)
        );
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                MachineAlternativeKey {
                    variant: 1,
                    ..alternative
                },
                &operands,
                &effects,
                &[0xe8, 0, 0, 0, 0],
                fixup,
            ),
            Err(X86_64ScalarCallTemplateError::AlternativeMismatch)
        );
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands[..2],
                &effects,
                &[0xe8, 0, 0, 0, 0],
                fixup,
            ),
            Err(X86_64ScalarCallTemplateError::OperandViewMismatch)
        );
        let mut malformed_effects = effects.clone();
        malformed_effects.external_operand_reads.clear();
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &malformed_effects,
                &[0xe8, 0, 0, 0, 0],
                fixup,
            ),
            Err(X86_64ScalarCallTemplateError::EffectMismatch)
        );
    }

    #[test]
    fn scalar_call_validation_rejects_nonzero_or_trailing_bytes_and_bad_fixup() {
        let (physical, kind, alternative, operands, effects) = inputs();
        let fixup = canonical_fixup(MachineId::new(7).unwrap());
        for malformed in [
            &[0xe9, 0, 0, 0, 0][..],
            &[0xe8, 1, 0, 0, 0][..],
            &[0xe8, 0, 0, 0][..],
            &[0xe8, 0, 0, 0, 0, 0][..],
        ] {
            assert_eq!(
                validate_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &effects,
                    malformed,
                    fixup,
                ),
                Err(X86_64ScalarCallTemplateError::MalformedTemplate)
            );
        }
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &[0xe8, 0, 0, 0, 0],
                X86_64ScalarCallFixup {
                    callee: MachineId::new(8).unwrap(),
                    ..fixup
                },
            ),
            Err(X86_64ScalarCallTemplateError::FixupMismatch)
        );
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &effects,
                &[0xe8, 0, 0, 0, 0],
                X86_64ScalarCallFixup {
                    patch_byte_offset: 0,
                    ..fixup
                },
            ),
            Err(X86_64ScalarCallTemplateError::FixupMismatch)
        );
    }
}
