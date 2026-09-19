//! Canonical unresolved AMD64 normalized-foreign-call templates.
//!
//! The emitted bytes are the same `E8 + zero rel32` placeholder an internal
//! call uses, but the fixup names the selected `{boundary, ordinal}` roster
//! row rather than an internal machine. The field stays unresolved until
//! object construction binds it to the declared import symbol; raw locator
//! bytes never enter this encoding contract.

use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

use crate::machine_effects::{X86_64SelectedAbi, x86_64_selected_abi};
use crate::{x86_64_physical_register_model, x86_64_register_constraint_catalog};

pub const X86_64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT: usize = 5;
pub const X86_64_NORMALIZED_FOREIGN_CALL_OPCODE_OFFSET: u16 = 0;
pub const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_OFFSET: u16 = 1;
pub const X86_64_NORMALIZED_FOREIGN_CALL_REFERENCE_OFFSET: u16 = 5;
pub const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupKind {
    Relative32FromNextInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; no raw foreign coordinate appears here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64NormalizedForeignCallFixup {
    pub kind: X86_64NormalizedForeignCallFixupKind,
    pub state: X86_64NormalizedForeignCallFixupState,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
}

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedNormalizedForeignCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: [u8; X86_64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT],
    fixup: X86_64NormalizedForeignCallFixup,
}

impl ValidatedX86_64SelectedNormalizedForeignCallTemplate {
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

    pub const fn bytes(&self) -> &[u8; X86_64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT] {
        &self.bytes
    }

    pub const fn fixup(&self) -> X86_64NormalizedForeignCallFixup {
        self.fixup
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for X86_64NormalizedForeignCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 normalized-foreign-call template: {self:?}"
        )
    }
}

impl std::error::Error for X86_64NormalizedForeignCallTemplateError {}

/// Produce `E8 + zero rel32` with an explicit unresolved import-field fixup.
pub fn encode_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let bytes = [0xe8, 0, 0, 0, 0];
    let fixup = canonical_fixup(boundary, ordinal);
    validate_x86_64_selected_normalized_foreign_call_template(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        effects,
        &bytes,
        fixup,
    )
}

/// Independently validate every selected input, byte, and unresolved fixup.
pub fn validate_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: X86_64NormalizedForeignCallFixup,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let abi = x86_64_selected_abi(target)
        .map_err(|_| X86_64NormalizedForeignCallTemplateError::UnsupportedTarget)?;
    if physical.model() != &x86_64_physical_register_model() {
        return Err(X86_64NormalizedForeignCallTemplateError::NonCanonicalPhysicalModel);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let expected_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::NormalizedForeignCall,
        variant: 0,
    };
    if alternative != expected_alternative {
        return Err(X86_64NormalizedForeignCallTemplateError::AlternativeMismatch);
    }
    // The exact selected constraint row owns the operand roster: find the
    // unique per-plan foreign row whose fixed views match the machine row.
    let keys = match abi {
        X86_64SelectedAbi::SystemV => crate::x86_64_system_v_normalized_foreign_call_keys(),
        X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_normalized_foreign_call_keys(),
    };
    let catalog = x86_64_register_constraint_catalog(physical);
    let row = catalog
        .constraints
        .iter()
        .find(|row| {
            keys.contains(&row.key)
                && row.operands.len() == operand_views.len()
                && row.operands.len() == operand_accesses.len()
                && row
                    .operands
                    .iter()
                    .zip(operand_views.iter().zip(operand_accesses))
                    .all(|(operand, (view, access))| {
                        operand.fixed_view == Some(*view) && operand.access == *access
                    })
        })
        .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    let stack_pointer = physical
        .model()
        .view_named("rsp")
        .expect("canonical x86-64 model contains rsp")
        .id;
    let expected = MachineEncodedEffects {
        external_operand_reads: row
            .operands
            .iter()
            .filter(|operand| operand.access == register_model::RegisterOperandAccess::Use)
            .map(|operand| operand.operand)
            .collect(),
        external_operand_writes: row
            .operands
            .iter()
            .filter(|operand| operand.access == register_model::RegisterOperandAccess::Def)
            .map(|operand| operand.operand)
            .collect(),
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
    };
    if effects != &expected {
        return Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch);
    }
    let bytes: [u8; X86_64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT] = bytes
        .try_into()
        .map_err(|_| X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    if bytes != [0xe8, 0, 0, 0, 0] {
        return Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    if fixup != canonical_fixup(boundary, ordinal) {
        return Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedX86_64SelectedNormalizedForeignCallTemplate {
        kind,
        alternative,
        operand_views: operand_views.to_vec(),
        effects: effects.clone(),
        bytes,
        fixup,
    })
}

fn canonical_fixup(boundary: BoundaryMachineId, ordinal: u32) -> X86_64NormalizedForeignCallFixup {
    X86_64NormalizedForeignCallFixup {
        kind: X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1,
        state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: X86_64_NORMALIZED_FOREIGN_CALL_OPCODE_OFFSET,
        patch_byte_offset: X86_64_NORMALIZED_FOREIGN_CALL_PATCH_OFFSET,
        reference_byte_offset: X86_64_NORMALIZED_FOREIGN_CALL_REFERENCE_OFFSET,
        patch_byte_width: X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64NormalizedForeignCallTemplateError, canonical_fixup,
        encode_x86_64_selected_normalized_foreign_call_template,
        validate_x86_64_selected_normalized_foreign_call_template,
    };
    use register_model::{
        RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel,
        validate_physical_register_model,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, SelectedInstructionKind,
    };
    use semantic_vocabulary::BoundaryMachineId;
    use target::NativeTarget;

    use crate::{x86_64_physical_register_model, x86_64_register_constraint_catalog};

    fn inputs(
        arity: usize,
        has_result: bool,
    ) -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        Vec<RegisterViewId>,
        Vec<RegisterOperandAccess>,
        MachineEncodedEffects,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 2,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        let catalog = x86_64_register_constraint_catalog(&physical);
        let key = crate::x86_64_system_v_normalized_foreign_call_keys()
            [arity * 2 + usize::from(has_result)];
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == key)
            .unwrap();
        let operands = row
            .operands
            .iter()
            .map(|operand| operand.fixed_view.unwrap())
            .collect::<Vec<_>>();
        let accesses = row
            .operands
            .iter()
            .map(|operand| operand.access)
            .collect::<Vec<_>>();
        let stack_pointer = physical.model().view_named("rsp").unwrap().id;
        let effects = MachineEncodedEffects {
            external_operand_reads: row
                .operands
                .iter()
                .filter(|operand| operand.access == register_model::RegisterOperandAccess::Use)
                .map(|operand| operand.operand)
                .collect(),
            external_operand_writes: row
                .operands
                .iter()
                .filter(|operand| operand.access == register_model::RegisterOperandAccess::Def)
                .map(|operand| operand.operand)
                .collect(),
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
        };
        (physical, kind, alternative, operands, accesses, effects)
    }

    #[test]
    fn template_has_exact_bytes_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects) = inputs(1, true);
        let template = encode_x86_64_selected_normalized_foreign_call_template(
            NativeTarget::linux_x64(),
            &physical,
            kind,
            alternative,
            &operands,
            &accesses,
            &effects,
        )
        .unwrap();
        assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
        assert_eq!(template.kind(), kind);
        assert_eq!(template.alternative(), alternative);
        assert_eq!(template.operand_views(), &operands);
        assert_eq!(template.effects(), &effects);
        let fixup = template.fixup();
        assert_eq!(
            fixup,
            canonical_fixup(BoundaryMachineId::new(3).unwrap(), 2)
        );
        assert_eq!(fixup.opcode_byte_offset, 0);
        assert_eq!(fixup.patch_byte_offset, 1);
        assert_eq!(fixup.reference_byte_offset, 5);
        assert_eq!(fixup.patch_byte_width, 4);
    }

    #[test]
    fn every_plan_row_selects_its_exact_operands_and_effects() {
        let (physical, kind, alternative, _, _, _) = inputs(0, false);
        for arity in 0..=6usize {
            for has_result in [false, true] {
                let (_, _, _, operands, accesses, effects) = inputs(arity, has_result);
                let template = encode_x86_64_selected_normalized_foreign_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                )
                .unwrap();
                assert_eq!(template.operand_views(), operands.as_slice());
                // A different plan's operand roster selects that plan's row,
                // whose canonical effects reject this row's effects.
                if arity > 0 {
                    let (_, _, _, wrong, wrong_accesses, _) = inputs(arity - 1, has_result);
                    assert!(
                        encode_x86_64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &wrong,
                            &wrong_accesses,
                            &effects,
                        )
                        .is_err()
                    );
                    // A view outside the integer argument bank matches no row.
                    let mut substituted = operands.clone();
                    substituted[0] = physical.model().view_named("r11").unwrap().id;
                    assert_eq!(
                        encode_x86_64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &substituted,
                            &accesses,
                            &effects,
                        ),
                        Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
                    );
                    // A swapped operand role selects no row for this plan.
                    let mut swapped_accesses = accesses.clone();
                    swapped_accesses[0] = RegisterOperandAccess::Def;
                    assert!(
                        encode_x86_64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &swapped_accesses,
                            &effects,
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn microsoft_rows_select_their_exact_operands() {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        let catalog = x86_64_register_constraint_catalog(&physical);
        // Microsoft x64 admits at most four register arguments.
        let key = crate::x86_64_microsoft_normalized_foreign_call_keys()[2 * 2 + 1];
        let row = catalog
            .constraints
            .iter()
            .find(|row| row.key == key)
            .unwrap();
        let operands = row
            .operands
            .iter()
            .map(|operand| operand.fixed_view.unwrap())
            .collect::<Vec<_>>();
        let accesses = row
            .operands
            .iter()
            .map(|operand| operand.access)
            .collect::<Vec<_>>();
        let stack_pointer = physical.model().view_named("rsp").unwrap().id;
        let effects = MachineEncodedEffects {
            external_operand_reads: row
                .operands
                .iter()
                .filter(|operand| operand.access == register_model::RegisterOperandAccess::Use)
                .map(|operand| operand.operand)
                .collect(),
            external_operand_writes: row
                .operands
                .iter()
                .filter(|operand| operand.access == register_model::RegisterOperandAccess::Def)
                .map(|operand| operand.operand)
                .collect(),
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
        };
        assert!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::windows_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
            )
            .is_ok()
        );
        // The System V roster cannot ride a Microsoft target.
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::windows_x64(),
                &physical,
                kind,
                alternative,
                &[physical.model().view_named("rdi").unwrap().id],
                &[RegisterOperandAccess::Use],
                &effects,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
        );
    }

    #[test]
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects) = inputs(1, true);
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                SelectedInstructionKind::CallScalar {
                    callee: semantic_vocabulary::MachineId::new(4).unwrap(),
                },
                alternative,
                &operands,
                &accesses,
                &effects,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch)
        );
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_arm64(),
            NativeTarget {
                object_format: target::ObjectFormat::MachO,
                ..NativeTarget::linux_x64()
            },
        ] {
            assert_eq!(
                encode_x86_64_selected_normalized_foreign_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                ),
                Err(X86_64NormalizedForeignCallTemplateError::UnsupportedTarget)
            );
        }
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                MachineAlternativeKey {
                    family: MachineAlternativeFamily::CallScalar,
                    variant: 0,
                },
                &operands,
                &accesses,
                &effects,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::AlternativeMismatch)
        );
        let template = encode_x86_64_selected_normalized_foreign_call_template(
            NativeTarget::linux_x64(),
            &physical,
            kind,
            alternative,
            &operands,
            &accesses,
            &effects,
        )
        .unwrap();
        // A substituted boundary or ordinal cannot validate.
        let mut foreign = template.fixup();
        foreign.boundary = BoundaryMachineId::new(9).unwrap();
        assert_eq!(
            validate_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                template.bytes(),
                foreign,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        foreign = template.fixup();
        foreign.ordinal = 7;
        assert_eq!(
            validate_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                template.bytes(),
                foreign,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        // The canonical placeholder is the only admitted byte sequence.
        assert_eq!(
            validate_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                &[0xe8, 0, 0, 0, 1],
                template.fixup(),
            ),
            Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)
        );
        // A complete effect mismatch rejects.
        let mut wrong_effects = effects.clone();
        wrong_effects.implicit_unit_clobbers.clear();
        assert_eq!(
            encode_x86_64_selected_normalized_foreign_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &wrong_effects,
            ),
            Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch)
        );
    }
}
