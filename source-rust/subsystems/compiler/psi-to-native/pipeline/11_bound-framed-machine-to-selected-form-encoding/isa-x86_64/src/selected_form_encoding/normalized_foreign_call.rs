mod callback_field;

pub(crate) use target_x86_64::form_effects::normalized_foreign_call::x86_64_machine_register_encoding;

use crate::machine_effects::x86_64_selected_abi;
use normalized_foreign_call_plan::{Fixup, Template};
use register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

const PATCH_WIDTH: u8 = 4;
const CALLBACK_RELOC_OFFSET: u16 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64CallbackMaterializationDestination {
    Register(calling_conventions::MachineRegister),
    OutgoingStack {
        byte_offset: u32,
    },
    RecordField {
        root_register: calling_conventions::MachineRegister,
        placements: calling_conventions::PrivateCallbackPlacements,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64CallbackAddressMaterialization {
    pub destination: X86_64CallbackMaterializationDestination,
    pub materialization_byte_offset: u16,
    pub materialization_byte_count: u16,
    pub relocation_byte_offset: u16,
}

pub type X86_64NormalizedForeignCallFixup = Fixup<
    X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState,
    X86_64CallbackAddressMaterialization,
>;
pub type ValidatedX86_64SelectedNormalizedForeignCallTemplate =
    Template<X86_64NormalizedForeignCallFixup>;
pub type X86_64NormalizedForeignCallTemplateError = normalized_foreign_call_plan::Error;

pub fn callback_group_bytes(
    destination: X86_64CallbackMaterializationDestination,
) -> Option<Vec<u8>> {
    match destination {
        X86_64CallbackMaterializationDestination::Register(reg) => {
            let idx = x86_64_machine_register_encoding(reg)?;
            Some(vec![
                0x48 | (u8::from(idx >= 8) << 2),
                0x8d,
                0x05 | ((idx & 7) << 3),
                0,
                0,
                0,
                0,
            ])
        }
        X86_64CallbackMaterializationDestination::OutgoingStack { byte_offset } => {
            if byte_offset > i32::MAX as u32 {
                return None;
            }
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0, 0x4c, 0x89];
            if byte_offset <= i8::MAX as u32 {
                group.extend_from_slice(&[0x5c, 0x24, byte_offset as u8]);
            } else {
                group.extend_from_slice(&[0x9c, 0x24]);
                group.extend_from_slice(&byte_offset.to_le_bytes());
            }
            Some(group)
        }
        X86_64CallbackMaterializationDestination::RecordField {
            root_register,
            placements,
        } => {
            let mut group = vec![0x4c, 0x8d, 0x1d, 0, 0, 0, 0];
            for &placement in placements.as_slice() {
                callback_field::append_fragment(
                    &mut group,
                    calling_conventions::MachineRegister::X86R11,
                    root_register,
                    calling_conventions::MachineRegister::X86R10,
                    calling_conventions::MachineRegister::X86Rax,
                    placement,
                )?;
            }
            (!placements.as_slice().is_empty()).then_some(group)
        }
    }
}

fn callback_fixup(
    boundary: BoundaryMachineId,
    ordinal: u32,
    destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallTemplateError> {
    let bad = X86_64NormalizedForeignCallTemplateError::MalformedTemplate;
    let mut callbacks = Vec::with_capacity(destinations.len());
    let mut offset = 0u16;
    for destination in destinations {
        let count = u16::try_from(callback_group_bytes(destination.clone()).ok_or(bad)?.len())
            .map_err(|_| bad)?;
        callbacks.push(X86_64CallbackAddressMaterialization {
            destination: destination.clone(),
            materialization_byte_offset: offset,
            materialization_byte_count: count,
            relocation_byte_offset: offset.checked_add(CALLBACK_RELOC_OFFSET).ok_or(bad)?,
        });
        offset = offset.checked_add(count).ok_or(bad)?;
    }
    Ok(X86_64NormalizedForeignCallFixup {
        kind: X86_64NormalizedForeignCallFixupKind::Relative32FromNextInstructionToNormalizedForeignImportV1,
        state: X86_64NormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_byte_offset: offset,
        patch_byte_offset: offset.checked_add(1).ok_or(bad)?,
        reference_byte_offset: offset.checked_add(5).ok_or(bad)?,
        patch_byte_width: PATCH_WIDTH,
        callbacks,
    })
}

fn build_bytes(
    destinations: &[X86_64CallbackMaterializationDestination],
) -> Result<Vec<u8>, X86_64NormalizedForeignCallTemplateError> {
    let mut bytes = Vec::new();
    for destination in destinations {
        bytes.extend_from_slice(
            &callback_group_bytes(destination.clone())
                .ok_or(X86_64NormalizedForeignCallTemplateError::MalformedTemplate)?,
        );
    }
    bytes.extend_from_slice(&[0xe8, 0, 0, 0, 0]);
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_x86_64_selected_normalized_foreign_call_template(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    callback_destinations: &[X86_64CallbackMaterializationDestination],
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
        return Err(X86_64NormalizedForeignCallTemplateError::InstructionKindMismatch);
    };
    let bytes = build_bytes(callback_destinations)?;
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
        record,
    )
}

#[allow(clippy::too_many_arguments)]
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
    record: &SelectedNormalizedForeignCall,
) -> Result<
    ValidatedX86_64SelectedNormalizedForeignCallTemplate,
    X86_64NormalizedForeignCallTemplateError,
> {
    let (boundary, ordinal) = normalized_foreign_call_plan::admit(
        target,
        physical,
        kind,
        alternative,
        operand_views,
        operand_accesses,
        effects,
        record,
        |native| x86_64_selected_abi(native).is_ok(),
        crate::canonical_x86_64_physical_register_model_identity,
        crate::machine_effects::normalized_foreign_constraint,
        crate::x86_64_normalized_foreign_call_declaration,
    )?;
    let destinations = record
        .call
        .callbacks
        .iter()
        .map(|cb| {
            match (
                &cb.destination,
                cb.application.placement.locations.as_slice(),
            ) {
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Some(X86_64CallbackMaterializationDestination::Register(
                    *register,
                )),
                (
                    calling_conventions::NativePlace::Parameter(_),
                    [
                        calling_conventions::ValueLocation::Stack {
                            stack_byte_offset, ..
                        },
                    ],
                ) => Some(X86_64CallbackMaterializationDestination::OutgoingStack {
                    byte_offset: *stack_byte_offset,
                }),
                (
                    calling_conventions::NativePlace::Field { .. },
                    [calling_conventions::ValueLocation::Register { register, .. }],
                ) => Some(X86_64CallbackMaterializationDestination::RecordField {
                    root_register: *register,
                    placements: cb.field_placements.clone()?,
                }),
                _ => None,
            }
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch)?;
    if destinations != callback_destinations {
        return Err(X86_64NormalizedForeignCallTemplateError::OperandViewMismatch);
    }
    if bytes != build_bytes(callback_destinations)? {
        return Err(X86_64NormalizedForeignCallTemplateError::MalformedTemplate);
    }
    if fixup != callback_fixup(boundary, ordinal, callback_destinations)? {
        return Err(X86_64NormalizedForeignCallTemplateError::FixupMismatch);
    }
    Ok(Template::new(
        kind,
        alternative,
        operand_views,
        effects,
        bytes,
        fixup,
    ))
}
