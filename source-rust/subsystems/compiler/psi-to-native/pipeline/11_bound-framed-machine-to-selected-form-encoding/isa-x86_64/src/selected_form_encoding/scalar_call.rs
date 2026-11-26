use crate::machine_effects::{X86_64SelectedAbi, x86_64_selected_abi};
use crate::{x86_64_register_constraint_catalog_for, x86_64_system_v_register_call_keys};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;

pub(crate) const X86_64_SCALAR_CALL_TEMPLATE_BYTE_COUNT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64ScalarCallFixupKind {
    Relative32FromNextInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64ScalarCallFixupState {
    UnresolvedZeroFieldV1,
}

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

wire::invalid_error_display!(X86_64ScalarCallTemplateError, "x86-64 scalar-call template");

fn canonical_fixup(callee: MachineId) -> X86_64ScalarCallFixup {
    X86_64ScalarCallFixup {
        kind: X86_64ScalarCallFixupKind::Relative32FromNextInstructionToInternalMachineV1,
        state: X86_64ScalarCallFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_byte_offset: 0,
        patch_byte_offset: 1,
        reference_byte_offset: 5,
        patch_byte_width: 4,
    }
}

fn expected_operand_views(
    abi: X86_64SelectedAbi,
    physical: &ValidatedPhysicalRegisterModel,
    arity: usize,
) -> Vec<RegisterViewId> {
    let args: &[&str] = match abi {
        X86_64SelectedAbi::SystemV => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
        X86_64SelectedAbi::Microsoft => &["rcx", "rdx", "r8", "r9"],
    };
    args.iter()
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
    let key = match abi {
        X86_64SelectedAbi::SystemV => x86_64_system_v_register_call_keys()[arity],
        X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_call_keys()[arity],
    };
    let row = catalog
        .constraints
        .iter()
        .find(|r| r.key == key)
        .expect("canonical scalar-call row");
    let rsp = physical
        .model()
        .view_named("rsp")
        .expect("canonical rsp")
        .id;
    let mut effects = crate::machine_effects::user_direct_call_effects(row, rsp);
    effects.external_operand_reads = (0..arity as u16).collect();
    effects.external_operand_writes = vec![arity as u16];
    effects
}

fn expected_no_normal_mechanics(
    abi: X86_64SelectedAbi,
    physical: &ValidatedPhysicalRegisterModel,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), X86_64ScalarCallTemplateError> {
    let ms = abi == X86_64SelectedAbi::Microsoft;
    let mut keys: Vec<_> = if ms {
        crate::x86_64_microsoft_register_call_keys()
            .into_iter()
            .chain(crate::x86_64_microsoft_register_unit_call_keys())
            .chain(crate::x86_64_microsoft_mixed_unit_call_keys())
            .chain(crate::register_model::x86_64_microsoft_aggregate_call_keys())
            .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
            .collect()
    } else {
        crate::x86_64_system_v_register_call_keys()
            .into_iter()
            .chain(crate::x86_64_system_v_register_unit_call_keys())
            .chain(crate::x86_64_system_v_mixed_unit_call_keys())
            .chain(crate::register_model::x86_64_system_v_aggregate_call_keys())
            .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
            .collect()
    };
    keys.extend(crate::x86_64_float_scalar_call_keys(ms));
    keys.extend(crate::x86_64_indirect_aggregate_call_keys(ms));
    let catalog = x86_64_register_constraint_catalog_for(physical);
    let rsp = physical
        .model()
        .view_named("rsp")
        .ok_or(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel)?
        .id;
    for row in &catalog.constraints {
        if !keys.contains(&row.key)
            || row.operands.len() != operand_views.len()
            || !row
                .operands
                .iter()
                .zip(operand_views)
                .all(|(o, v)| o.fixed_view == Some(*v))
        {
            continue;
        }
        let expected = crate::machine_effects::user_direct_call_effects(row, rsp);
        if &expected == effects {
            return Ok((operand_views.to_vec(), expected));
        }
    }
    Err(X86_64ScalarCallTemplateError::EffectMismatch)
}

fn expected_call_mechanics(
    abi: X86_64SelectedAbi,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    operand_views: &[RegisterViewId],
) -> Result<(Vec<RegisterViewId>, MachineEncodedEffects), X86_64ScalarCallTemplateError> {
    let unit = matches!(kind, SelectedInstructionKind::CallUnit { .. });
    let aggregate = matches!(kind, SelectedInstructionKind::CallAggregate { .. });
    let float_scalar = !unit
        && !aggregate
        && operand_views.iter().any(|id| {
            physical
                .model()
                .views
                .iter()
                .any(|v| v.id == *id && v.name.starts_with("xmm"))
        });
    if unit || aggregate || float_scalar {
        let ms = abi == X86_64SelectedAbi::Microsoft;
        let keys: Vec<_> = if aggregate {
            if ms {
                crate::register_model::x86_64_microsoft_aggregate_call_keys()
                    .into_iter()
                    .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                    .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                    .collect()
            } else {
                crate::register_model::x86_64_system_v_aggregate_call_keys()
                    .into_iter()
                    .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                    .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                    .collect()
            }
        } else if float_scalar {
            crate::x86_64_float_scalar_call_keys(ms)
        } else if ms {
            crate::x86_64_microsoft_register_unit_call_keys()
                .into_iter()
                .chain(crate::x86_64_microsoft_mixed_unit_call_keys())
                .collect()
        } else {
            crate::x86_64_system_v_register_unit_call_keys()
                .into_iter()
                .chain(crate::x86_64_system_v_mixed_unit_call_keys())
                .collect()
        };
        let catalog = x86_64_register_constraint_catalog_for(physical);
        let row = catalog
            .constraints
            .iter()
            .find(|r| {
                keys.contains(&r.key)
                    && r.operands.len() == operand_views.len()
                    && r.operands
                        .iter()
                        .zip(operand_views)
                        .all(|(o, v)| o.fixed_view == Some(*v))
            })
            .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
        let mut expected = expected_effects(abi, physical, 0);
        expected.external_operand_reads = row.operand_reads();
        expected.external_operand_writes = row.operand_writes();
        expected.implicit_unit_uses = row.implicit_uses.clone();
        expected.implicit_unit_defs = row.implicit_defs.clone();
        expected.implicit_unit_clobbers = row.clobbers.clone();
        Ok((operand_views.to_vec(), expected))
    } else {
        let max_arity = match abi {
            X86_64SelectedAbi::SystemV => 6,
            X86_64SelectedAbi::Microsoft => 4,
        };
        let arity = operand_views
            .len()
            .checked_sub(1)
            .filter(|&a| a <= max_arity)
            .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
        let views = expected_operand_views(abi, physical, arity);
        if operand_views != views {
            return Err(X86_64ScalarCallTemplateError::OperandViewMismatch);
        }
        Ok((views, expected_effects(abi, physical, arity)))
    }
}

pub fn encode_x86_64_selected_scalar_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
) -> Result<ValidatedX86_64SelectedScalarCallTemplate, X86_64ScalarCallTemplateError> {
    let callee = kind
        .machine_callee()
        .ok_or(X86_64ScalarCallTemplateError::InstructionKindMismatch)?;
    validate_x86_64_selected_scalar_call_template(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        effects,
        &crate::ORDINARY_DIRECT_CALL_BYTES,
        canonical_fixup(callee),
    )
}

#[allow(clippy::too_many_arguments)]
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
    let abi = x86_64_selected_abi(target)
        .map_err(|_| X86_64ScalarCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel);
    }
    let callee = kind
        .machine_callee()
        .ok_or(X86_64ScalarCallTemplateError::InstructionKindMismatch)?;
    if alternative != kind.call_alternative_key() {
        return Err(X86_64ScalarCallTemplateError::AlternativeMismatch);
    }
    let (expected_views, expected) =
        if matches!(kind, SelectedInstructionKind::InvokeNoNormal { .. }) {
            expected_no_normal_mechanics(abi, physical, operand_views, effects)?
        } else {
            expected_call_mechanics(abi, physical, kind, operand_views)?
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
        operand_views: expected_views,
        effects: effects.clone(),
        bytes,
        fixup,
    })
}

pub fn encode_x86_64_selected_dynamic_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    effects: &MachineEncodedEffects,
) -> Result<(Vec<u8>, MachineEncodedEffects), X86_64ScalarCallTemplateError> {
    let abi = x86_64_selected_abi(target)
        .map_err(|_| X86_64ScalarCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64ScalarCallTemplateError::NonCanonicalPhysicalModel);
    }
    let offset = kind
        .call_target()
        .and_then(|t| t.indirect_code_offset())
        .ok_or(X86_64ScalarCallTemplateError::InstructionKindMismatch)?;
    let family = match kind {
        SelectedInstructionKind::CallUnit { .. } => MachineAlternativeFamily::CallUnit,
        SelectedInstructionKind::CallScalar { .. } => MachineAlternativeFamily::CallScalar,
        SelectedInstructionKind::CallAggregate { .. } => MachineAlternativeFamily::CallAggregate,
        _ => return Err(X86_64ScalarCallTemplateError::InstructionKindMismatch),
    };
    if alternative != (MachineAlternativeKey { family, variant: 1 }) {
        return Err(X86_64ScalarCallTemplateError::AlternativeMismatch);
    }
    let selector = physical
        .model()
        .view_named("r11")
        .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?
        .id;
    let mut ordinary = operand_views.to_vec();
    let pos = ordinary
        .iter()
        .position(|v| *v == selector)
        .ok_or(X86_64ScalarCallTemplateError::OperandViewMismatch)?;
    ordinary.remove(pos);
    let (_, baseline) = expected_call_mechanics(abi, physical, kind, &ordinary)?;
    if pos != baseline.external_operand_reads.len() {
        return Err(X86_64ScalarCallTemplateError::OperandViewMismatch);
    }
    let expected = selected_instructions::calls::dynamic::encoded_effects(
        &baseline,
        physical.model().architecture,
    )
    .ok_or(X86_64ScalarCallTemplateError::EffectMismatch)?;
    if *effects != expected {
        return Err(X86_64ScalarCallTemplateError::EffectMismatch);
    }
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&[0x49, 0xba]);
    bytes.extend_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(&[
        0x4f, 0x8b, 0x1c, 0x13, 0x4d, 0x85, 0xdb, 0x75, 0x02, 0x0f, 0x0b, 0x41, 0xff, 0xd3,
    ]);
    Ok((bytes, expected))
}
