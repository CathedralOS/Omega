//! Canonical unresolved AMD64 ordinary register scalar-call templates.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

use crate::machine_effects::{X86_64SelectedAbi, x86_64_selected_abi};
use crate::{x86_64_register_constraint_catalog_for, x86_64_system_v_register_call_keys};

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
    operand_views: Vec<RegisterViewId>,
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

    pub fn operand_views(&self) -> &[RegisterViewId] {
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
        SelectedInstructionKind::CallScalar { callee }
        | SelectedInstructionKind::CallAggregate { callee }
        | SelectedInstructionKind::CallUnit { callee } => callee,
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
    // The scalar-call encoding matrix is the declared (architecture,
    // object-format) pair matrix in `machine_effects`: (X86_64, Elf) selects
    // System-V rosters and (X86_64, Coff) selects Microsoft x64 rosters.
    // Undeclared pairs — (X86_64, Mach-O) and every other architecture — fail
    // closed here rather than falling through an else arm into one family.
    let abi = x86_64_selected_abi(target)
        .map_err(|_| X86_64ScalarCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel);
    }
    let callee = match kind {
        SelectedInstructionKind::CallScalar { callee }
        | SelectedInstructionKind::CallAggregate { callee }
        | SelectedInstructionKind::CallUnit { callee } => callee,
        _ => return Err(X86_64ScalarCallTemplateError::InstructionKindMismatch),
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
        return Err(X86_64ScalarCallTemplateError::AlternativeMismatch);
    }
    let aggregate = matches!(kind, SelectedInstructionKind::CallAggregate { .. });
    let floating_scalar = !unit
        && !aggregate
        && operand_views.iter().any(|id| {
            physical
                .model()
                .views
                .iter()
                .any(|view| view.id == *id && view.name.starts_with("xmm"))
        });
    let (expected_operand_views, expected) = if unit || aggregate || floating_scalar {
        let keys = if aggregate {
            match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                        .collect()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                        .collect()
                }
            }
        } else if floating_scalar {
            crate::x86_64_float_scalar_call_keys(abi == X86_64SelectedAbi::Microsoft)
        } else {
            match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_register_unit_call_keys()
                    .into_iter()
                    .chain(crate::x86_64_system_v_mixed_unit_call_keys())
                    .collect::<Vec<_>>(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_unit_call_keys()
                    .into_iter()
                    .chain(crate::x86_64_microsoft_mixed_unit_call_keys())
                    .collect::<Vec<_>>(),
            }
        };
        let catalog = x86_64_register_constraint_catalog_for(physical);
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
            .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
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
            .filter(|arity| {
                *arity
                    <= match abi {
                        X86_64SelectedAbi::SystemV => 6,
                        X86_64SelectedAbi::Microsoft => 4,
                    }
            })
            .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
        let expected_operand_views = expected_operand_views(abi, physical, arity);
        if operand_views != expected_operand_views {
            return Err(X86_64ScalarCallTemplateError::OperandViewMismatch);
        }
        (
            expected_operand_views,
            expected_effects(abi, physical, arity),
        )
    };
    if effects != &expected {
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

fn expected_operand_views(
    abi: X86_64SelectedAbi,
    physical: &ValidatedPhysicalRegisterModel,
    arity: usize,
) -> Vec<RegisterViewId> {
    let arguments: &[&str] = match abi {
        X86_64SelectedAbi::SystemV => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
        X86_64SelectedAbi::Microsoft => &["rcx", "rdx", "r8", "r9"],
    };
    arguments
        .iter()
        .copied()
        .take(arity)
        .chain(["rax"])
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
    abi: X86_64SelectedAbi,
    physical: &ValidatedPhysicalRegisterModel,
    arity: usize,
) -> MachineEncodedEffects {
    let catalog = x86_64_register_constraint_catalog_for(physical);
    let row = catalog
        .constraints
        .iter()
        .find(|row| {
            row.key
                == match abi {
                    X86_64SelectedAbi::SystemV => x86_64_system_v_register_call_keys()[arity],
                    X86_64SelectedAbi::Microsoft => {
                        crate::x86_64_microsoft_register_call_keys()[arity]
                    }
                }
        })
        .expect("canonical x86-64 catalog contains scalar-call constraint");
    let stack_pointer = physical
        .model()
        .view_named("rsp")
        .expect("canonical x86-64 model contains rsp")
        .id;
    MachineEncodedEffects {
        external_operand_reads: (0..arity as u16).collect(),
        external_operand_writes: vec![arity as u16],
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
mod mixed_calls;
#[cfg(test)]
mod target_abis;
#[cfg(test)]
mod unit_calls;

#[cfg(test)]
mod tests {
    use super::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, MachineId,
        NativeTarget, RegisterViewId, SelectedInstructionKind, ValidatedPhysicalRegisterModel,
        X86_64ScalarCallFixup, X86_64ScalarCallTemplateError, X86_64SelectedAbi, canonical_fixup,
        encode_x86_64_selected_scalar_call_template, expected_effects, expected_operand_views,
        validate_x86_64_selected_scalar_call_template, x86_64_selected_abi,
        x86_64_system_v_register_call_keys,
    };
    use crate::x86_64_physical_register_model;
    mod mixed_aggregates;
    use register_model::validate_physical_register_model;

    fn inputs() -> (
        ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        Vec<RegisterViewId>,
        MachineEncodedEffects,
    ) {
        let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::CallScalar {
            callee: MachineId::new(7).unwrap(),
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::CallScalar,
            variant: 0,
        };
        let operands = expected_operand_views(X86_64SelectedAbi::SystemV, &physical, 2);
        let effects = expected_effects(X86_64SelectedAbi::SystemV, &physical, 2);
        (physical, kind, alternative, operands, effects)
    }

    #[test]
    fn every_register_arity_has_exact_operands_and_effects() {
        let (physical, kind, alternative, _, _) = inputs();
        let catalog = crate::x86_64_register_constraint_catalog(&physical);
        for (arity, key) in x86_64_system_v_register_call_keys().into_iter().enumerate() {
            let row = catalog
                .constraints
                .iter()
                .find(|row| row.key == key)
                .unwrap();
            let operands = expected_operand_views(X86_64SelectedAbi::SystemV, &physical, arity);
            assert_eq!(row.operands.len(), arity + 1);
            assert_eq!(
                row.operands
                    .iter()
                    .map(|operand| operand.fixed_view.unwrap())
                    .collect::<Vec<_>>(),
                operands
            );
            let effects = expected_effects(X86_64SelectedAbi::SystemV, &physical, arity);
            assert_eq!(
                effects.external_operand_reads,
                (0..arity as u16).collect::<Vec<_>>()
            );
            assert_eq!(effects.external_operand_writes, vec![arity as u16]);
            let template = encode_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
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
                encode_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &wrong_effects,
                )
                .is_err()
            );
            let mut wrong_views = operands.clone();
            wrong_views[arity] = physical.model().view_named("rsp").unwrap().id;
            assert!(
                encode_x86_64_selected_scalar_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &wrong_views,
                    &effects,
                )
                .is_err()
            );
        }
        let oversized = vec![physical.model().view_named("rax").unwrap().id; 8];
        assert_eq!(
            encode_x86_64_selected_scalar_call_template(
                NativeTarget::linux_x64(),
                &physical,
                kind,
                alternative,
                &oversized,
                &expected_effects(X86_64SelectedAbi::SystemV, &physical, 0),
            ),
            Err(X86_64ScalarCallTemplateError::OperandViewMismatch)
        );
    }

    #[test]
    fn scalar_call_abi_matrix_resolves_declared_pairs_and_fails_closed() {
        assert_eq!(
            x86_64_selected_abi(NativeTarget::linux_x64()),
            Ok(X86_64SelectedAbi::SystemV)
        );
        for declared_coff in [NativeTarget::windows_x64(), NativeTarget::uefi_x64()] {
            assert_eq!(
                x86_64_selected_abi(declared_coff),
                Ok(X86_64SelectedAbi::Microsoft)
            );
        }
        for undeclared in [
            NativeTarget {
                object_format: target::ObjectFormat::MachO,
                ..NativeTarget::linux_x64()
            },
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            assert_eq!(
                x86_64_selected_abi(undeclared),
                Err(
                    crate::machine_effects::X86_64MachineEffectCatalogValidationError::UnsupportedTargetAbi,
                )
            );
        }
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
    fn microsoft_direct_aggregate_templates_replay_registers_and_clobbers() {
        let (physical, _, _, _, _) = inputs();
        let constraints = crate::validate_x86_64_register_constraint_catalog(
            crate::x86_64_register_constraint_catalog(&physical),
            &physical,
        )
        .unwrap();
        let native = NativeTarget::windows_x64();
        let catalog = crate::x86_64_machine_effect_catalog(native, &constraints).unwrap();
        let kind = SelectedInstructionKind::CallAggregate {
            callee: MachineId::new(7).unwrap(),
        };
        for (arity, key) in crate::register_model::x86_64_microsoft_aggregate_call_keys()
            .into_iter()
            .enumerate()
        {
            let declaration = catalog
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.constraint == key
                        && declaration.semantic
                            == selected_instructions::MachineSemanticKind::CallAggregate
                })
                .unwrap();
            let alternative = &declaration.alternatives[0];
            let operands =
                expected_operand_views(x86_64_selected_abi(native).unwrap(), &physical, arity);
            assert_eq!(
                alternative.encoded.external_operand_reads,
                (0..arity as u16).collect::<Vec<_>>()
            );
            assert_eq!(
                alternative.encoded.external_operand_writes,
                vec![arity as u16]
            );
            let template = encode_x86_64_selected_scalar_call_template(
                native,
                &physical,
                kind,
                alternative.key,
                &operands,
                &alternative.encoded,
            )
            .unwrap();
            assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
            let mut changed = alternative.encoded.clone();
            changed.implicit_unit_clobbers.clear();
            assert!(
                encode_x86_64_selected_scalar_call_template(
                    native,
                    &physical,
                    kind,
                    alternative.key,
                    &operands,
                    &changed,
                )
                .is_err()
            );
            let mut wrong_registers = operands.clone();
            wrong_registers[arity] = physical.model().view_named("rdx").unwrap().id;
            assert!(
                encode_x86_64_selected_scalar_call_template(
                    native,
                    &physical,
                    kind,
                    alternative.key,
                    &wrong_registers,
                    &alternative.encoded,
                )
                .is_err()
            );
        }
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
        // Undeclared (architecture, object-format) pairs fail closed through
        // the same matrix: an x86-64 Mach-O target does not inherit either
        // declared row.
        let undeclared_macho = NativeTarget {
            object_format: target::ObjectFormat::MachO,
            ..NativeTarget::linux_x64()
        };
        assert_eq!(
            validate_x86_64_selected_scalar_call_template(
                undeclared_macho,
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
