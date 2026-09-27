use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use target::Architecture;

use machine_code::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormMachineDisposition,
    SelectedFormNormalizedForeignCallFixup, SelectedFormNormalizedForeignCallFixupKind,
    SelectedFormNormalizedForeignCallFixupState,
};

use super::super::{OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence};
use super::branch;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    widened: bool,
) -> Result<
    (
        Vec<u8>,
        Option<Box<ResolvedBranchEvidence>>,
        Option<SelectedFormInternalMachineFixup>,
        Option<SelectedFormNormalizedForeignCallFixup>,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (&pre.machine_disposition, &pre.state) {
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => Ok((bytes.clone(), None, None, None)),
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedInternalMachineCall { bytes, fixup, .. },
        ) => Ok((
            bytes.clone(),
            None,
            Some(validate_internal_fixup(
                architecture,
                instruction,
                instruction_offset,
                bytes,
                fixup.clone(),
            )?),
            None,
        )),
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { bytes, fixup, .. },
        ) => Ok((
            bytes.clone(),
            None,
            None,
            Some(validate_foreign_fixup(
                architecture,
                instruction,
                instruction_offset,
                bytes,
                fixup.clone(),
            )?),
        )),
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => branch::resolve(
            architecture,
            block,
            instruction,
            instruction_offset,
            block_offsets,
            machine,
            physical,
            widened,
        )
        .map(|(bytes, branch)| (bytes, branch, None, None)),
        _ => unexpected(instruction.id),
    }
}

fn validate_internal_fixup(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    bytes: &[u8],
    fixup: SelectedFormInternalMachineFixup,
) -> Result<SelectedFormInternalMachineFixup, OptimizedResolvedSelectedFormLayoutError> {
    let (SelectedInstructionKind::CallScalar { callee }
    | SelectedInstructionKind::CallUnit { callee }
    | SelectedInstructionKind::CallAggregate { callee }) = instruction.kind
    else {
        return unexpected(instruction.id);
    };
    match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
        ) => {}
        (
            Architecture::Aarch64,
            SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        ) => {}
        _ => return unexpected(instruction.id),
    }
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || fixup.addend != 0
    {
        return unexpected(instruction.id);
    }
    let patch_start = usize::from(fixup.patch_row_offset);
    let patch_end = patch_start
        .checked_add(usize::from(fixup.patch_byte_width))
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    let canonical_placeholder = match fixup.kind {
        SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 => {
            bytes.get(patch_start..patch_end) == Some(&[0, 0, 0, 0])
        }
        SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => {
            bytes.get(patch_start..patch_end) == Some(&0x9400_0000_u32.to_le_bytes())
        }
    };
    if !canonical_placeholder {
        return unexpected(instruction.id);
    }
    let row_len = u64::try_from(bytes.len())
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    let row_end = instruction_offset
        .checked_add(row_len)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if instruction_offset
        .checked_add(u64::from(fixup.opcode_row_offset))
        .is_none_or(|offset| offset >= row_end)
        || instruction_offset
            .checked_add(u64::from(fixup.reference_row_offset))
            .is_none_or(|offset| offset > row_end)
    {
        return unexpected(instruction.id);
    }
    Ok(fixup)
}

/// The foreign-call field stays unresolved through layout: validate the
/// `{boundary, ordinal}` roster binding and the canonical zero placeholder,
/// then carry the row-relative fixup unchanged.
fn validate_foreign_fixup(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    bytes: &[u8],
    fixup: SelectedFormNormalizedForeignCallFixup,
) -> Result<SelectedFormNormalizedForeignCallFixup, OptimizedResolvedSelectedFormLayoutError> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind
    else {
        return unexpected(instruction.id);
    };
    match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
        ) => {}
        (
            Architecture::Aarch64,
            SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        ) => {}
        _ => return unexpected(instruction.id),
    }
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.boundary != boundary
        || fixup.ordinal != ordinal
        || fixup.addend != 0
    {
        return unexpected(instruction.id);
    }
    let patch_start = usize::from(fixup.patch_row_offset);
    let patch_end = patch_start
        .checked_add(usize::from(fixup.patch_byte_width))
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    let canonical_placeholder = match fixup.kind {
        SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => {
            bytes.get(patch_start..patch_end) == Some(&[0, 0, 0, 0])
        }
        SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
            bytes.get(patch_start..patch_end) == Some(&0x9400_0000_u32.to_le_bytes())
        }
    };
    if !canonical_placeholder {
        return unexpected(instruction.id);
    }
    let row_len = u64::try_from(bytes.len())
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    let row_end = instruction_offset
        .checked_add(row_len)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    if instruction_offset
        .checked_add(u64::from(fixup.opcode_row_offset))
        .is_none_or(|offset| offset >= row_end)
        || instruction_offset
            .checked_add(u64::from(fixup.reference_row_offset))
            .is_none_or(|offset| offset > row_end)
    {
        return unexpected(instruction.id);
    }
    validate_callback_materializations(
        architecture,
        instruction,
        bytes,
        fixup.opcode_row_offset,
        &fixup.callbacks,
    )?;
    Ok(fixup)
}

/// The materialization groups travel inside the call row's encoding: they
/// own the row from offset 0 up to the call opcode, in fixup roster order,
/// and every relocation placeholder inside them stays zeroed against the
/// canonical template.
fn validate_callback_materializations(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    bytes: &[u8],
    opcode_row_offset: u16,
    callbacks: &[machine_code::SelectedFormCallbackMaterialization],
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let row_len = u64::try_from(bytes.len())
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
    let mut cursor = 0_u64;
    for callback in callbacks {
        let materialization_end = u64::from(callback.code_row_offset)
            .checked_add(u64::from(callback.code_byte_count))
            .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        if u64::from(callback.code_row_offset) != cursor
            || materialization_end > row_len
            || materialization_end > u64::from(opcode_row_offset)
        {
            return unexpected(instruction.id);
        }
        let group = bytes
            .get(cursor as usize..materialization_end as usize)
            .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        let zero_patch = |offset: u16| -> Result<bool, OptimizedResolvedSelectedFormLayoutError> {
            let start = usize::from(offset)
                .checked_sub(cursor as usize)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
            let end = start
                .checked_add(4)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
            Ok(u64::from(offset) >= cursor
                && u64::from(offset).checked_add(4) <= Some(materialization_end)
                && group.get(start..end) == Some(&[0, 0, 0, 0]))
        };
        match (architecture, callback.encoding) {
            (
                Architecture::X86_64,
                machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                    patch_row_offset,
                },
            ) => {
                let canonical = match callback.destination {
                    machine_code::CallbackAddressDestination::Register(_) => {
                        // REX 8D modrm disp32 — seven bytes, disp32 patch at +3.
                        callback.code_byte_count == 7
                            && patch_row_offset as u64 == cursor + 3
                            && group.len() == 7
                            && group[0] & 0xfe == 0x48
                            && group[1] == 0x8d
                            && group[2] & 0xc7 == 0x05
                            && zero_patch(patch_row_offset)?
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => {
                        // lea r11, [rip+disp32] then mov [root+off], r11 —
                        // fourteen bytes, disp32 patch at +3.
                        if !matches!(
                            root_register,
                            calling_conventions::MachineRegister::X86Rax
                                | calling_conventions::MachineRegister::X86Rcx
                                | calling_conventions::MachineRegister::X86Rdx
                                | calling_conventions::MachineRegister::X86Rbx
                                | calling_conventions::MachineRegister::X86Rsi
                                | calling_conventions::MachineRegister::X86Rdi
                                | calling_conventions::MachineRegister::X86R8
                                | calling_conventions::MachineRegister::X86R9
                                | calling_conventions::MachineRegister::X86R10
                                | calling_conventions::MachineRegister::X86R11
                                | calling_conventions::MachineRegister::X86R12
                                | calling_conventions::MachineRegister::X86R13
                                | calling_conventions::MachineRegister::X86R14
                                | calling_conventions::MachineRegister::X86R15
                        ) {
                            return unexpected(instruction.id);
                        }
                        let root = root_register.codec_index();
                        callback.code_byte_count == 14
                            && patch_row_offset as u64 == cursor + 3
                            && group.len() == 14
                            && group[0] == 0x4c
                            && group[1] == 0x8d
                            && group[2] == 0x1d
                            && group[7] == 0x4c | u8::from(root >= 8)
                            && group[8] == 0x89
                            && group[9] == 0x98 | (root & 7)
                            && u32::from_le_bytes([group[10], group[11], group[12], group[13]])
                                == byte_offset
                            && zero_patch(patch_row_offset)?
                    }
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        return unexpected(instruction.id);
                    }
                };
                if !canonical {
                    return unexpected(instruction.id);
                }
            }
            (
                Architecture::Aarch64,
                machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                    page_patch_row_offset,
                    page_offset_patch_row_offset,
                },
            ) => {
                let canonical = match callback.destination {
                    machine_code::CallbackAddressDestination::Register(_) => {
                        callback.code_byte_count == 8
                            && page_patch_row_offset as u64 == cursor
                            && page_offset_patch_row_offset as u64 == cursor + 4
                            && group.len() == 8
                            && group[1] == 0
                            && group[2] == 0
                            && group[3] == 0x90
                            && group[0] & 0xe0 == 0
                            && group[5] & 0xfc == 0
                            && group[6] == 0
                            && group[7] == 0x91
                            && group[4] & 0x1f == group[0] & 0x1f
                            && group[4] >> 5 == group[0] & 7
                            && group[5] & 3 == group[0] >> 3 & 3
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => {
                        let calling_conventions::MachineRegister::Aarch64X(root) = root_register
                        else {
                            return unexpected(instruction.id);
                        };
                        let store = group
                            .get(8..12)
                            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
                        callback.code_byte_count == 12
                            && byte_offset % 8 == 0
                            && page_patch_row_offset as u64 == cursor
                            && page_offset_patch_row_offset as u64 == cursor + 4
                            && group.len() == 12
                            && group[0] == 16
                            && group[1] == 0
                            && group[2] == 0
                            && group[3] == 0x90
                            && group[4] == 0x10
                            && group[5] == 0x02
                            && group[6] == 0
                            && group[7] == 0x91
                            && store
                                == Some(
                                    0xF900_0000_u32
                                        | ((byte_offset / 8) << 10)
                                        | (u32::from(root) << 5)
                                        | 16,
                                )
                    }
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        return unexpected(instruction.id);
                    }
                };
                if !canonical {
                    return unexpected(instruction.id);
                }
            }
            _ => return unexpected(instruction.id),
        }
        cursor = materialization_end;
    }
    if u64::from(opcode_row_offset) != cursor {
        return unexpected(instruction.id);
    }
    Ok(())
}

fn unexpected<T>(
    instruction: SelectedInstructionId,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction))
}
