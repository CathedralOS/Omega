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
use crate::x86_64_register_constraint_catalog_for;

pub const X86_64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT: usize = 5;
pub const X86_64_NORMALIZED_FOREIGN_CALL_OPCODE_OFFSET: u16 = 0;
pub const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_OFFSET: u16 = 1;
pub const X86_64_NORMALIZED_FOREIGN_CALL_REFERENCE_OFFSET: u16 = 5;
pub const X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;

/// Callback-bearing rows place one address-materialization group per private
/// callback destination before the call itself: a `lea <destination>,
/// [rip+disp32]` for a private parameter slot, or a `lea r11,
/// [rip+disp32]` + `mov [root+offset], r11` pair for a record-field
/// destination — then the canonical 5-byte call.
pub const X86_64_CALLBACK_MATERIALIZATION_BYTE_COUNT: u16 = 7;
pub const X86_64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT: u16 = 14;
pub const X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET: u16 = 3;

/// Where one callback materialization's resolved private-function address
/// lands: an ABI parameter register, or a private field inside the record a
/// register-resident root parameter names (encoded through scratch `r11`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    RecordField {
        root_register: calling_conventions::MachineRegister,
        byte_offset: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupKind {
    Relative32FromNextInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Target-owned private-callback materialization record: a `lea
/// <destination>, [rip+disp32]` prefix (optionally followed by the
/// record-field slot store) whose displacement field binds the selected
/// private function symbol at object construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64CallbackAddressMaterialization {
    pub destination: X86_64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub relocation_byte_offset: u16,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; no raw foreign coordinate appears here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64NormalizedForeignCallFixup {
    pub kind: X86_64NormalizedForeignCallFixupKind,
    pub state: X86_64NormalizedForeignCallFixupState,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
    /// One entry per callback destination the call's private schedule names,
    /// in evaluated roster order.
    pub callbacks: Vec<X86_64CallbackAddressMaterialization>,
}

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64SelectedNormalizedForeignCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: Vec<u8>,
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

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn fixup(&self) -> &X86_64NormalizedForeignCallFixup {
        &self.fixup
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
/// `callback_destinations` names each private callback destination in the
/// call's evaluated roster order; every entry emits its materialization
/// group ahead of the call bytes and lands a custody record in the fixup.
pub fn encode_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let mut bytes = Vec::new();
    for destination in callback_destinations {
        bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    let fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
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
        callback_destinations,
    )
}

/// Independently validate every selected input, byte, and unresolved fixup.
/// `callback_destinations` is the same evaluated roster the encoder received:
/// its shape must agree with the constraint row's declared callback shape.
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
    callback_destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let abi = x86_64_selected_abi(target)
        .map_err(|_| X86_64NormalizedForeignCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64NormalizedForeignCallTemplateError::NonCanonicalPhysicalModel);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let catalog = x86_64_register_constraint_catalog_for(physical);
    let row = {
        let keys = match abi {
            X86_64SelectedAbi::SystemV => crate::x86_64_system_v_normalized_foreign_call_keys(),
            X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_normalized_foreign_call_keys(),
        };
        // Callback families share explicit operands across the catalog — a
        // private parameter slot is implicit-only and the field scratch adds
        // `r11` uses — so the row must join on implicit uses too; matching
        // operands alone would resolve the plain family for a callback row.
        catalog
            .constraints
            .iter()
            .find(|row| {
                keys.contains(&row.key)
                    && row.implicit_uses == effects.implicit_unit_uses
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
            .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?
    };
    let shape =
        crate::register_model::x86_64_normalized_foreign_callback_shape(row, physical.model());
    let expected_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::NormalizedForeignCall,
        variant: shape.alternative_variant(),
    };
    if alternative != expected_alternative {
        return Err(X86_64NormalizedForeignCallTemplateError::AlternativeMismatch);
    }
    // The descriptor roster must agree with the row's declared shape: a
    // register destination only where the row pins a private slot register,
    // field destinations only where the row declares the scratch, and at
    // most one register destination per call.
    let mut register_destinations = 0;
    for destination in callback_destinations {
        match destination {
            X86_64CallbackMaterializationDestination::Register(register) => {
                if shape.parameter_slot != Some(*register) {
                    return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
                }
                register_destinations += 1;
            }
            X86_64CallbackMaterializationDestination::RecordField { root_register, .. } => {
                if !shape.field_scratch
                    || x86_64_machine_register_encoding(*root_register).is_none()
                {
                    return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
                }
            }
        }
    }
    if register_destinations > 1 || (register_destinations == 0) != shape.parameter_slot.is_none() {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    if callback_destinations.iter().any(|destination| {
        matches!(
            destination,
            X86_64CallbackMaterializationDestination::RecordField { .. }
        )
    }) != shape.field_scratch
    {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
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
    let mut expected_bytes = Vec::new();
    for destination in callback_destinations {
        expected_bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    expected_bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    if bytes != expected_bytes.as_slice() {
        return Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)?;
    if fixup != expected_fixup {
        return Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedX86_64SelectedNormalizedForeignCallTemplate {
        kind,
        alternative,
        operand_views: operand_views.to_vec(),
        effects: effects.clone(),
        bytes: bytes.to_vec(),
        fixup,
    })
}

/// One materialization group: `REX.W 8D /r` — `lea <destination>,
/// [rip+disp32]` — for a private parameter slot, or `lea r11,
/// [rip+disp32]` followed by `mov [root+disp32], r11` for a record-field
/// destination. The lea's zero disp32 is the relocation field; the field
/// store's displacement is the literal private-slot offset.
fn callback_group_bytes(destination: X86_64CallbackMaterializationDestination) -> Option<Vec<u8>> {
    match destination {
        X86_64CallbackMaterializationDestination::Register(register) => {
            let index = x86_64_machine_register_encoding(register)?;
            let rex = 0x48 | (u8::from(index >= 8) << 2);
            let modrm = 0x05 | ((index & 7) << 3);
            Some(vec![rex, 0x8d, modrm, 0, 0, 0, 0])
        }
        X86_64CallbackMaterializationDestination::RecordField {
            root_register,
            byte_offset,
        } => {
            let root = x86_64_machine_register_encoding(root_register)?;
            // `4C 8D 1D` — lea r11, [rip+disp32] — then `4C|B 89 /r` storing
            // r11 at [root + byte_offset] with a mod=10 disp32 form, which
            // covers every encodable root register without a SIB byte.
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0];
            let rex = 0x4c | u8::from(root >= 8);
            let modrm = 0x98 | (root & 7);
            group.extend_from_slice(&[rex, 0x89, modrm]);
            group.extend_from_slice(&byte_offset.to_le_bytes());
            Some(group)
        }
    }
}

fn x86_64_machine_register_encoding(register: calling_conventions::MachineRegister) -> Option<u8> {
    Some(match register {
        calling_conventions::MachineRegister::X86Rax => 0,
        calling_conventions::MachineRegister::X86Rcx => 1,
        calling_conventions::MachineRegister::X86Rdx => 2,
        calling_conventions::MachineRegister::X86Rbx => 3,
        calling_conventions::MachineRegister::X86Rsi => 6,
        calling_conventions::MachineRegister::X86Rdi => 7,
        calling_conventions::MachineRegister::X86R8 => 8,
        calling_conventions::MachineRegister::X86R9 => 9,
        calling_conventions::MachineRegister::X86R10 => 10,
        calling_conventions::MachineRegister::X86R11 => 11,
        calling_conventions::MachineRegister::X86R12 => 12,
        calling_conventions::MachineRegister::X86R13 => 13,
        calling_conventions::MachineRegister::X86R14 => 14,
        calling_conventions::MachineRegister::X86R15 => 15,
        _ => return None,
    })
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallTemplateError> {
    let mut callbacks = Vec::with_capacity(destinations.len());
    let mut offset = 0_u16;
    for destination in destinations {
        let byte_count = match destination {
            X86_64CallbackMaterializationDestination::Register(_) => {
                X86_64_CALLBACK_MATERIALIZATION_BYTE_COUNT
            }
            X86_64CallbackMaterializationDestination::RecordField { .. } => {
                X86_64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT
            }
        };
        callbacks.push(X86_64CallbackAddressMaterialization {
            destination: *destination,
            materialization_byte_offset: offset,
            materialization_byte_count: byte_count,
            relocation_byte_offset: offset
                .checked_add(X86_64_CALLBACK_MATERIALIZATION_RELOCATION_OFFSET)
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        });
        offset = offset
            .checked_add(byte_count)
            .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    }
    let patch_byte_offset = offset
        .checked_add(1)
        .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    let reference_byte_offset = offset
        .checked_add(5)
        .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?;
    Ok(X86_64NormalizedForeignCallFixup {
        kind: X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1,
        state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset,
        reference_byte_offset,
        patch_byte_width: X86_64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64NormalizedForeignCallTemplateError, callback_fixup,
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
        input_layout: usize,
        result: usize,
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
        let key = crate::x86_64_system_v_normalized_foreign_call_keys()[input_layout * 3 + result];
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
        let (physical, kind, alternative, operands, accesses, effects) = inputs(1, 1);
        let template = encode_x86_64_selected_normalized_foreign_call_template(
            NativeTarget::linux_x64(),
            &physical,
            kind,
            alternative,
            &operands,
            &accesses,
            &effects,
            &[],
        )
        .unwrap();
        assert_eq!(template.bytes(), &[0xe8, 0, 0, 0, 0]);
        assert_eq!(template.kind(), kind);
        assert_eq!(template.alternative(), alternative);
        assert_eq!(template.operand_views(), &operands);
        assert_eq!(template.effects(), &effects);
        let fixup = template.fixup().clone();
        assert_eq!(
            fixup,
            callback_fixup(BoundaryMachineId::new(3).unwrap(), 2, &[]).unwrap()
        );
        assert_eq!(fixup.opcode_byte_offset, 0);
        assert_eq!(fixup.patch_byte_offset, 1);
        assert_eq!(fixup.reference_byte_offset, 5);
        assert_eq!(fixup.patch_byte_width, 4);
    }

    #[test]
    fn every_plan_row_selects_its_exact_operands_and_effects() {
        let (physical, kind, alternative, _, _, _) = inputs(0, 0);
        for input_layout in 0..63 {
            for result in 0..3 {
                let (_, _, _, operands, accesses, effects) = inputs(input_layout, result);
                let template = encode_x86_64_selected_normalized_foreign_call_template(
                    NativeTarget::linux_x64(),
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                    &[],
                )
                .unwrap();
                assert_eq!(template.operand_views(), operands.as_slice());
                // A different result choice is a valid row, but cannot retain
                // this row's external writes and remaining caller-save effects.
                let (_, _, _, wrong, wrong_accesses, _) = inputs(input_layout, (result + 1) % 3);
                assert_eq!(
                    encode_x86_64_selected_normalized_foreign_call_template(
                        NativeTarget::linux_x64(),
                        &physical,
                        kind,
                        alternative,
                        &wrong,
                        &wrong_accesses,
                        &effects,
                        &[],
                    ),
                    Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch)
                );
                if !operands.is_empty() {
                    // This view belongs to neither the input nor result bank.
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
                            &[],
                        ),
                        Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
                    );
                    // Flip the actual role: result-only rows start with Def,
                    // whereas nonempty input layouts start with Use. If the
                    // new roles describe another valid row, stale effects reject.
                    let mut swapped_accesses = accesses.clone();
                    swapped_accesses[0] = match accesses[0] {
                        RegisterOperandAccess::Use => RegisterOperandAccess::Def,
                        RegisterOperandAccess::Def => RegisterOperandAccess::Use,
                        RegisterOperandAccess::UseDef => unreachable!(),
                    };
                    assert!(
                        encode_x86_64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_x64(),
                            &physical,
                            kind,
                            alternative,
                            &operands,
                            &swapped_accesses,
                            &effects,
                            &[],
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
                &[],
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
                &[],
            ),
            Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)
        );
    }

    #[test]
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects) = inputs(1, 1);
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
                &[],
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
                    &[],
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
                &[],
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
            &[],
        )
        .unwrap();
        // A substituted boundary or ordinal cannot validate.
        let mut foreign = template.fixup().clone();
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
                &[],
            ),
            Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        foreign = template.fixup().clone();
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
                &[],
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
                template.fixup().clone(),
                &[],
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
                &[],
            ),
            Err(X86_64NormalizedForeignCallTemplateError::EffectMismatch)
        );
    }
}
