use super::plan::unexpected;
use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionKind,
};
use target::Architecture;

use machine_code::{
    CallbackAddressDestination, DeferredControlEncodingReason, ImageAddressFields,
    SelectedFormCallbackMaterializationEncoding, SelectedFormEncodingRow,
    SelectedFormEncodingState, SelectedFormImageAddressFixup, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};

use super::super::OptimizedResolvedSelectedFormLayoutError as Error;
use super::super::ResolvedBranchEvidence;
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
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<
    (
        Vec<u8>,
        Option<Box<ResolvedBranchEvidence>>,
        Option<Box<SelectedFormInternalMachineFixup>>,
        Option<Box<SelectedFormNormalizedForeignCallFixup>>,
        Option<Box<SelectedFormImageAddressFixup>>,
    ),
    Error,
> {
    // The resolved row owns its own byte vector; it is filled from the
    // encoding row's interned span rather than from a per-row vector.
    let bytes = encoded.bytes(pre.code);
    match (&pre.machine_disposition, &pre.state) {
        (SelectedFormMachineDisposition::RetainedV1, SelectedFormEncodingState::Encoded) => {
            Ok((bytes.to_vec(), None, None, None, None))
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup },
        ) => Ok((
            bytes.to_vec(),
            None,
            Some(Box::new(validate_internal_fixup(
                architecture,
                instruction,
                instruction_offset,
                bytes,
                **fixup,
            )?)),
            None,
            None,
        )),
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup },
        ) => Ok((
            bytes.to_vec(),
            None,
            None,
            Some(Box::new(validate_foreign_fixup(
                architecture,
                instruction,
                instruction_offset,
                bytes,
                fixup.as_ref().clone(),
            )?)),
            None,
        )),
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedImageAddress { fixup },
        ) => Ok((
            bytes.to_vec(),
            None,
            None,
            None,
            Some(Box::new(validate_image_address_fixup(
                instruction,
                bytes,
                **fixup,
            )?)),
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
        .map(|(bytes, branch)| (bytes, branch, None, None, None)),
        _ => unexpected(instruction.id),
    }
}

/// The image address field stays unresolved through layout: bind the
/// fixup to the instruction's exact referent and keep its fields inside the row.
fn validate_image_address_fixup(
    instruction: &SelectedInstruction,
    bytes: &[u8],
    fixup: SelectedFormImageAddressFixup,
) -> Result<SelectedFormImageAddressFixup, Error> {
    let SelectedInstructionKind::ImageAddress { reference } = instruction.kind else {
        return unexpected(instruction.id);
    };
    let row_len = u64::try_from(bytes.len()).map_err(|_| Error::OffsetOverflow)?;
    let fields_inside = match fixup.fields {
        ImageAddressFields::X86_64Relative32 { patch_offset } => {
            patch_offset.checked_add(4) == Some(row_len)
        }
        ImageAddressFields::Aarch64PageAddress {
            page_patch_offset,
            page_offset_patch_offset,
        } => {
            page_patch_offset.checked_add(4) == Some(page_offset_patch_offset)
                && page_offset_patch_offset.checked_add(4) == Some(row_len)
        }
    };
    if fixup.reference != reference || !fields_inside {
        return unexpected(instruction.id);
    }
    Ok(fixup)
}

fn validate_internal_fixup(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    bytes: &[u8],
    fixup: SelectedFormInternalMachineFixup,
) -> Result<SelectedFormInternalMachineFixup, Error> {
    let Some(callee) = instruction.kind.machine_callee() else {
        return unexpected(instruction.id);
    };
    let placeholder = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
        ) => [0, 0, 0, 0],
        (
            Architecture::Aarch64,
            SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        ) => 0x9400_0000_u32.to_le_bytes(),
        _ => return unexpected(instruction.id),
    };
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || fixup.addend != 0
    {
        return unexpected(instruction.id);
    }
    validate_unresolved_field(
        instruction,
        instruction_offset,
        bytes,
        fixup.patch_row_offset,
        fixup.patch_byte_width,
        fixup.opcode_row_offset,
        fixup.reference_row_offset,
        placeholder,
    )?;
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
) -> Result<SelectedFormNormalizedForeignCallFixup, Error> {
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind
    else {
        return unexpected(instruction.id);
    };
    let placeholder = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
        ) => [0, 0, 0, 0],
        (
            Architecture::Aarch64,
            SelectedFormNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        ) => 0x9400_0000_u32.to_le_bytes(),
        _ => return unexpected(instruction.id),
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.boundary != boundary
        || fixup.ordinal != ordinal
        || fixup.addend != 0
    {
        return unexpected(instruction.id);
    }
    validate_unresolved_field(
        instruction,
        instruction_offset,
        bytes,
        fixup.patch_row_offset,
        fixup.patch_byte_width,
        fixup.opcode_row_offset,
        fixup.reference_row_offset,
        placeholder,
    )?;
    validate_callback_materializations(
        architecture,
        instruction,
        bytes,
        fixup.opcode_row_offset,
        &fixup.callbacks,
    )?;
    Ok(fixup)
}

/// Both unresolved field families keep the same custody shape: the patch still
/// holds its canonical template at the row-relative offset, and the opcode and
/// reference offsets stay inside the row the field patches.
#[allow(clippy::too_many_arguments)]
fn validate_unresolved_field(
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    bytes: &[u8],
    patch_row_offset: u16,
    patch_byte_width: u8,
    opcode_row_offset: u16,
    reference_row_offset: u16,
    placeholder: [u8; 4],
) -> Result<(), Error> {
    let patch_start = usize::from(patch_row_offset);
    let patch_end = patch_start
        .checked_add(usize::from(patch_byte_width))
        .ok_or(Error::OffsetOverflow)?;
    if bytes.get(patch_start..patch_end) != Some(placeholder.as_slice()) {
        return unexpected(instruction.id);
    }
    let row_len = u64::try_from(bytes.len()).map_err(|_| Error::OffsetOverflow)?;
    let row_end = instruction_offset
        .checked_add(row_len)
        .ok_or(Error::OffsetOverflow)?;
    let opcode = instruction_offset.checked_add(u64::from(opcode_row_offset));
    let reference = instruction_offset.checked_add(u64::from(reference_row_offset));
    if opcode.is_none_or(|at| at >= row_end) || reference.is_none_or(|at| at > row_end) {
        return unexpected(instruction.id);
    }
    Ok(())
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
) -> Result<(), Error> {
    let row_len = u64::try_from(bytes.len()).map_err(|_| Error::OffsetOverflow)?;
    let mut cursor = 0_u64;
    for callback in callbacks {
        let materialization_end = u64::from(callback.code_row_offset)
            .checked_add(u64::from(callback.code_byte_count))
            .ok_or(Error::OffsetOverflow)?;
        if u64::from(callback.code_row_offset) != cursor
            || materialization_end > row_len
            || materialization_end > u64::from(opcode_row_offset)
        {
            return unexpected(instruction.id);
        }
        let group = bytes
            .get(cursor as usize..materialization_end as usize)
            .ok_or(Error::OffsetOverflow)?;
        let zero_patch = |offset: u16| -> Result<bool, Error> {
            let start = usize::from(offset)
                .checked_sub(cursor as usize)
                .ok_or(Error::OffsetOverflow)?;
            let end = start.checked_add(4).ok_or(Error::OffsetOverflow)?;
            Ok(u64::from(offset) >= cursor
                && u64::from(offset).checked_add(4) <= Some(materialization_end)
                && group.get(start..end) == Some(&[0, 0, 0, 0]))
        };
        match (architecture, callback.encoding) {
            (
                Architecture::X86_64,
                SelectedFormCallbackMaterializationEncoding::X86_64Relative32 { patch_row_offset },
            ) => {
                let destination = match callback.destination.clone() {
                    CallbackAddressDestination::Register(register) => {
                        isa_x86_64::X86_64CallbackMaterializationDestination::Register(register)
                    }
                    CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    } => isa_x86_64::X86_64CallbackMaterializationDestination::RecordField {
                        root_register,
                        placements,
                    },
                    CallbackAddressDestination::OutgoingStack { byte_offset } => {
                        isa_x86_64::X86_64CallbackMaterializationDestination::OutgoingStack {
                            byte_offset,
                        }
                    }
                };
                let canonical = isa_x86_64::encode_private_callback_materialization(destination)
                    .is_some_and(|expected| group == expected)
                    && patch_row_offset as u64 == cursor + 3
                    && zero_patch(patch_row_offset)?;
                if !canonical {
                    return unexpected(instruction.id);
                }
            }
            (
                Architecture::Aarch64,
                SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                    page_patch_row_offset,
                    page_offset_patch_row_offset,
                },
            ) => {
                let destination = match callback.destination.clone() {
                    CallbackAddressDestination::Register(register) => {
                        isa_aarch64::Aarch64CallbackMaterializationDestination::Register(register)
                    }
                    CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    } => isa_aarch64::Aarch64CallbackMaterializationDestination::RecordField {
                        root_register,
                        placements,
                    },
                    CallbackAddressDestination::OutgoingStack { .. } => {
                        return unexpected(instruction.id);
                    }
                };
                let canonical = isa_aarch64::encode_private_callback_materialization(&destination)
                    .is_some_and(|expected| group == expected)
                    && page_patch_row_offset as u64 == cursor
                    && page_offset_patch_row_offset as u64 == cursor + 4;
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
