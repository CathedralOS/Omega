//! Canonical unresolved AArch64 normalized-foreign-call templates.
//!
//! The emitted word is the same `BL #0` placeholder an internal call uses,
//! but the fixup names the selected `{boundary, ordinal}` roster row rather
//! than an internal machine. Private callbacks materialize their function
//! address before the call word: one `adrp`/`add` pair per register
//! destination, and that pair plus a `str` through the architecture-fixed
//! scratch register for a record-field destination. The field stays
//! unresolved until object construction binds it to the declared import
//! symbol; raw locator bytes never enter this encoding contract.

use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, SelectedInstructionKind,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

use crate::aarch64_register_constraint_catalog_for;
use crate::machine_effects::{Aarch64SelectedAbi, aarch64_selected_abi};

pub const AARCH64_NORMALIZED_FOREIGN_CALL_TEMPLATE_BYTE_COUNT: usize = 4;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_OPCODE_OFFSET: u16 = 0;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_OFFSET: u16 = 0;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_REFERENCE_OFFSET: u16 = 0;
pub const AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH: u8 = 4;

/// One register-destination materialization group: an `adrp`/`add` pair.
/// A record-field destination adds the scratch store word after its pair.
pub const AARCH64_CALLBACK_MATERIALIZATION_BYTE_COUNT: u16 = 8;
pub const AARCH64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT: u16 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallFixupKind {
    BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallFixupState {
    UnresolvedImportFieldV1,
}

/// Where one private callback address lands when the call executes: the
/// parameter register the provider plan assigned, or a field inside the
/// record addressed by `root_register` at `byte_offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    RecordField {
        root_register: calling_conventions::MachineRegister,
        byte_offset: u32,
    },
}

/// Target-owned private-callback materialization record: an `adrp`/`add`
/// pair whose page and page-offset fields bind the selected private function
/// symbol at object construction, followed by the scratch store when the
/// destination is a record field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64CallbackAddressMaterialization {
    pub destination: Aarch64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub page_relocation_byte_offset: u16,
    pub page_offset_relocation_byte_offset: u16,
}

/// Target-owned unresolved import-field fixup for one normalized foreign
/// call. `{boundary, ordinal}` names the selected roster row owning the
/// evaluated locator; `callbacks` carries every private-callback
/// materialization in the call's roster order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64NormalizedForeignCallFixup {
    pub kind: Aarch64NormalizedForeignCallFixupKind,
    pub state: Aarch64NormalizedForeignCallFixupState,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
    pub callbacks: Vec<Aarch64CallbackAddressMaterialization>,
}

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SelectedNormalizedForeignCallTemplate {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: Vec<u8>,
    fixup: Aarch64NormalizedForeignCallFixup,
}

impl ValidatedAarch64SelectedNormalizedForeignCallTemplate {
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

    pub const fn fixup(&self) -> &Aarch64NormalizedForeignCallFixup {
        &self.fixup
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64NormalizedForeignCallTemplateError {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for Aarch64NormalizedForeignCallTemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 normalized-foreign-call template: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64NormalizedForeignCallTemplateError {}

/// Produce the canonical `BL #0` word, preceded by one materialization
/// group per callback destination in roster order, with an unresolved
/// import-field fixup naming every group.
pub fn encode_aarch64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[Aarch64CallbackMaterializationDestination],
) -> Result<
    ValidatedAarch64SelectedNormalizedForeignCallTemplate,
    Aarch64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    let mut bytes = Vec::new();
    for destination in callback_destinations {
        bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?,
        );
    }
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
    let fixup = callback_fixup(boundary, ordinal, callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)?;
    validate_aarch64_selected_normalized_foreign_call_template(
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
pub fn validate_aarch64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    bytes: &[u8],
    fixup: Aarch64NormalizedForeignCallFixup,
    callback_destinations: &[Aarch64CallbackMaterializationDestination],
) -> Result<
    ValidatedAarch64SelectedNormalizedForeignCallTemplate,
    Aarch64NormalizedForeignCallTemplateError,
> {
    let abi = aarch64_selected_abi(target)
        .map_err(|_| Aarch64NormalizedForeignCallTemplateError::UnsupportedTarget)?;
    if physical.identity() != crate::canonical_aarch64_physical_register_model_identity() {
        return Err(Aarch64NormalizedForeignCallTemplateError::NonCanonicalPhysicalModel);
    }
    let (boundary, ordinal) = match kind {
        SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } => (boundary, ordinal),
        _ => return Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch),
    };
    // The exact selected constraint row owns the operand roster: find the
    // unique per-plan foreign row whose fixed views match the machine row.
    let keys = match abi {
        Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_normalized_foreign_call_keys(),
        Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_normalized_foreign_call_keys(),
    };
    let catalog = aarch64_register_constraint_catalog_for(physical);
    // Callback families share explicit operands across the catalog — a
    // private parameter slot is implicit-only and the field scratch adds
    // uses — so the row must join on implicit uses too; matching operands
    // alone would resolve the plain family for a callback row.
    let row = catalog
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
        .ok_or(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    let shape =
        crate::register_model::aarch64_normalized_foreign_callback_shape(row, physical.model());
    let expected_alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::NormalizedForeignCall,
        variant: shape.alternative_variant(),
    };
    if alternative != expected_alternative {
        return Err(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch);
    }
    // The row's declared shape must agree with the destination descriptors:
    // at most one register destination, pinned to the parameter-slot
    // register the row removed from its operand bank.
    let mut register_destinations = 0;
    let mut field_destinations = 0;
    for destination in callback_destinations {
        match *destination {
            Aarch64CallbackMaterializationDestination::Register(register) => {
                register_destinations += 1;
                if Some(register) != shape.parameter_slot {
                    return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
                }
            }
            Aarch64CallbackMaterializationDestination::RecordField {
                root_register,
                byte_offset,
            } => {
                field_destinations += 1;
                if !shape.field_scratch
                    || callback_group_bytes(
                        Aarch64CallbackMaterializationDestination::RecordField {
                            root_register,
                            byte_offset,
                        },
                    )
                    .is_none()
                {
                    return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
                }
            }
        }
    }
    if register_destinations > 1
        || (field_destinations > 0 && !shape.field_scratch)
        || (field_destinations == 0 && shape.field_scratch)
    {
        return Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
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
        memory: MachineEncodedMemoryEffect::NoneV1,
        stack: MachineEncodedStackEffect::UnchangedV1,
        trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        control: MachineEncodedControlEffect::DirectRelativeCallV1,
    };
    if effects != &expected {
        return Err(Aarch64NormalizedForeignCallTemplateError::EffectMismatch);
    }
    let mut expected_bytes = Vec::new();
    for destination in callback_destinations {
        expected_bytes.extend_from_slice(
            &callback_group_bytes(*destination)
                .ok_or(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    expected_bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
    if bytes != expected_bytes.as_slice() {
        return Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    let expected_fixup = callback_fixup(boundary, ordinal, callback_destinations)
        .ok_or(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)?;
    if fixup != expected_fixup {
        return Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(ValidatedAarch64SelectedNormalizedForeignCallTemplate {
        kind,
        alternative,
        operand_views: operand_views.to_vec(),
        effects: effects.clone(),
        bytes: bytes.to_vec(),
        fixup,
    })
}

/// One callback's materialization group: `adrp`/`add` pair into the
/// destination register — or the scratch `x16` for a field destination —
/// followed by `str x16, [xRoot, #off]` for field destinations.
fn callback_group_bytes(destination: Aarch64CallbackMaterializationDestination) -> Option<Vec<u8>> {
    match destination {
        Aarch64CallbackMaterializationDestination::Register(register) => {
            let calling_conventions::MachineRegister::Aarch64X(index) = register else {
                return None;
            };
            if index > 30 {
                return None;
            }
            let adrp = (0x9000_0000_u32 | u32::from(index)).to_le_bytes();
            let add = (0x9100_0000_u32 | (u32::from(index) << 5) | u32::from(index)).to_le_bytes();
            Some(vec![
                adrp[0], adrp[1], adrp[2], adrp[3], add[0], add[1], add[2], add[3],
            ])
        }
        Aarch64CallbackMaterializationDestination::RecordField {
            root_register,
            byte_offset,
        } => {
            let calling_conventions::MachineRegister::Aarch64X(root_index) = root_register else {
                return None;
            };
            if root_index > 30 || byte_offset % 8 != 0 || byte_offset / 8 >= 4096 {
                return None;
            }
            let scratch = 16_u32;
            let adrp = (0x9000_0000_u32 | scratch).to_le_bytes();
            let add = (0x9100_0000_u32 | (scratch << 5) | scratch).to_le_bytes();
            let store = (0xF900_0000_u32
                | ((byte_offset / 8) << 10)
                | (u32::from(root_index) << 5)
                | scratch)
                .to_le_bytes();
            Some(vec![
                adrp[0], adrp[1], adrp[2], adrp[3], add[0], add[1], add[2], add[3], store[0],
                store[1], store[2], store[3],
            ])
        }
    }
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[Aarch64CallbackMaterializationDestination],
) -> Option<Aarch64NormalizedForeignCallFixup> {
    let mut materializations = Vec::with_capacity(destinations.len());
    let mut offset = 0_u16;
    for destination in destinations {
        let byte_count = match destination {
            Aarch64CallbackMaterializationDestination::Register(_) => {
                AARCH64_CALLBACK_MATERIALIZATION_BYTE_COUNT
            }
            Aarch64CallbackMaterializationDestination::RecordField { .. } => {
                AARCH64_CALLBACK_FIELD_MATERIALIZATION_BYTE_COUNT
            }
        };
        materializations.push(Aarch64CallbackAddressMaterialization {
            destination: *destination,
            materialization_byte_offset: offset,
            materialization_byte_count: byte_count,
            page_relocation_byte_offset: offset,
            page_offset_relocation_byte_offset: offset + 4,
        });
        offset = offset.checked_add(byte_count)?;
    }
    Some(Aarch64NormalizedForeignCallFixup {
        kind: Aarch64NormalizedForeignCallFixupKind::BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        state: Aarch64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset: offset,
        reference_byte_offset: offset,
        patch_byte_width: AARCH64_NORMALIZED_FOREIGN_CALL_PATCH_WIDTH,
        callbacks: materializations,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Aarch64NormalizedForeignCallTemplateError, callback_fixup,
        encode_aarch64_selected_normalized_foreign_call_template,
        validate_aarch64_selected_normalized_foreign_call_template,
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

    use crate::{aarch64_physical_register_model, aarch64_register_constraint_catalog};

    fn inputs(
        darwin: bool,
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
        let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
        let kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            ordinal: 2,
        };
        let alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::NormalizedForeignCall,
            variant: 0,
        };
        let catalog = aarch64_register_constraint_catalog(&physical);
        let key = if darwin {
            crate::aarch64_darwin_normalized_foreign_call_keys()
        } else {
            crate::aarch64_aapcs64_normalized_foreign_call_keys()
        }[input_layout * 3 + result];
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
            memory: MachineEncodedMemoryEffect::NoneV1,
            stack: MachineEncodedStackEffect::UnchangedV1,
            trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
            control: MachineEncodedControlEffect::DirectRelativeCallV1,
        };
        (physical, kind, alternative, operands, accesses, effects)
    }

    #[test]
    fn template_has_exact_bytes_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects) = inputs(true, 1, 1);
        let template = encode_aarch64_selected_normalized_foreign_call_template(
            NativeTarget::macos_arm64(),
            &physical,
            kind,
            alternative,
            &operands,
            &accesses,
            &effects,
            &[],
        )
        .unwrap();
        assert_eq!(template.bytes(), &0x9400_0000_u32.to_le_bytes());
        assert_eq!(template.kind(), kind);
        assert_eq!(template.alternative(), alternative);
        assert_eq!(template.operand_views(), &operands);
        assert_eq!(template.effects(), &effects);
        let fixup = template.fixup().clone();
        assert_eq!(
            fixup,
            callback_fixup(BoundaryMachineId::new(3).unwrap(), 2, &[]).unwrap()
        );
        assert_eq!(fixup.patch_byte_offset, 0);
        assert_eq!(fixup.patch_byte_width, 4);
    }

    #[test]
    fn every_plan_row_selects_its_exact_operands_and_effects() {
        let (physical, kind, alternative, _, _, _) = inputs(false, 0, 0);
        for input_layout in 0..81 {
            for result in 0..3 {
                let (_, _, _, operands, accesses, effects) = inputs(false, input_layout, result);
                let template = encode_aarch64_selected_normalized_foreign_call_template(
                    NativeTarget::linux_arm64(),
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
                let (_, _, _, wrong, wrong_accesses, _) =
                    inputs(false, input_layout, (result + 1) % 3);
                assert_eq!(
                    encode_aarch64_selected_normalized_foreign_call_template(
                        NativeTarget::linux_arm64(),
                        &physical,
                        kind,
                        alternative,
                        &wrong,
                        &wrong_accesses,
                        &effects,
                        &[],
                    ),
                    Err(Aarch64NormalizedForeignCallTemplateError::EffectMismatch)
                );
                if !operands.is_empty() {
                    // This view belongs to neither the input nor result bank.
                    let mut substituted = operands.clone();
                    substituted[0] = physical.model().view_named("x8").unwrap().id;
                    assert_eq!(
                        encode_aarch64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_arm64(),
                            &physical,
                            kind,
                            alternative,
                            &substituted,
                            &accesses,
                            &effects,
                            &[],
                        ),
                        Err(Aarch64NormalizedForeignCallTemplateError::OperandViewMismatch)
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
                        encode_aarch64_selected_normalized_foreign_call_template(
                            NativeTarget::linux_arm64(),
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
    fn rejects_wrong_kind_target_alternative_and_fixup() {
        let (physical, kind, alternative, operands, accesses, effects) = inputs(true, 1, 1);
        assert_eq!(
            encode_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::InstructionKindMismatch)
        );
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget {
                object_format: target::ObjectFormat::Coff,
                ..NativeTarget::macos_arm64()
            },
        ] {
            assert_eq!(
                encode_aarch64_selected_normalized_foreign_call_template(
                    target,
                    &physical,
                    kind,
                    alternative,
                    &operands,
                    &accesses,
                    &effects,
                    &[],
                ),
                Err(Aarch64NormalizedForeignCallTemplateError::UnsupportedTarget)
            );
        }
        assert_eq!(
            encode_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::AlternativeMismatch)
        );
        let template = encode_aarch64_selected_normalized_foreign_call_template(
            NativeTarget::macos_arm64(),
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
            validate_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        foreign = template.fixup().clone();
        foreign.ordinal = 7;
        assert_eq!(
            validate_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
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
            Err(Aarch64NormalizedForeignCallTemplateError::FixupMismatch)
        );
        // The canonical placeholder is the only admitted word.
        assert_eq!(
            validate_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &effects,
                &0x1400_0000_u32.to_le_bytes(),
                template.fixup().clone(),
                &[],
            ),
            Err(Aarch64NormalizedForeignCallTemplateError::MalformedTemplate)
        );
        // A complete effect mismatch rejects.
        let mut wrong_effects = effects.clone();
        wrong_effects.implicit_unit_clobbers.clear();
        assert_eq!(
            encode_aarch64_selected_normalized_foreign_call_template(
                NativeTarget::macos_arm64(),
                &physical,
                kind,
                alternative,
                &operands,
                &accesses,
                &wrong_effects,
                &[],
            ),
            Err(Aarch64NormalizedForeignCallTemplateError::EffectMismatch)
        );
    }
}
