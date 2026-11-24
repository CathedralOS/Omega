use machine_code::layout_requirements::*;
/*
We construct function-relative block and instruction layouts for ordinary functions in stage 12. We select the program-wide layout policy (`select`), order blocks along mandatory conditional fallthrough chains while keeping storage positions independent of sparse block identifiers (`derive_order`), iteratively widen AArch64 conditional branches whose taken displacement exceeds the signed 19-bit word range (`derive_plan`), and resolve each row's byte vector, branch evidence, and call or image-address fixup bounds (`layout`).
*/

use std::collections::BTreeMap;

use machine_code::{
    CallbackAddressDestination, DeferredControlEncodingReason, ImageAddressFields,
    SelectedFormCallbackMaterialization, SelectedFormCallbackMaterializationEncoding,
    SelectedFormEncodedRowStore, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormImageAddressFixup, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use target::Architecture;

use crate::{
    OptimizedResolvedSelectedFormLayoutError as Error, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedJumpEvidence, ResolvedSelectedBlockLayout,
    ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
    declared_size_matches,
};

pub(crate) use encoding_admission::layout::geometry::{
    ConditionalBranchDescriptor, byte_displacement, conditional_branch_descriptor,
    conditional_branch_family, derive_plan, instructions, select, unexpected,
};

pub(crate) fn derive_order(
    function: &EncodedLayoutFunction,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&EncodedLayoutBlock>, Error> {
    let invalid = || Error::UnsupportedFunctionShape(function.machine);
    let count = function.blocks.len();
    let mut positions: Vec<_> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id, position))
        .collect();
    if !positions.is_sorted_by_key(|(identity, _)| *identity) {
        positions.sort_unstable_by_key(|(identity, _)| *identity);
    }
    if positions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(invalid());
    }
    let position_of = |identity| {
        positions
            .binary_search_by_key(&identity, |(identity, _)| *identity)
            .ok()
            .map(|position| positions[position].1)
    };
    let entry = position_of(function.entry_block).ok_or_else(invalid)?;
    if policy == SelectedFunctionLayoutPolicy::SingleEntryBlockV1
        && (count != 1
            || !matches!(
                function.blocks[entry].terminator,
                EncodedLayoutTerminator::Return { .. }
            ))
    {
        return Err(invalid());
    }
    let mut following = vec![None; count];
    let mut preceding = vec![None; count];
    for (source, block) in function.blocks.iter().enumerate() {
        let successor =
            conditional_branch_descriptor(block).map(|descriptor| descriptor.when_fallthrough);
        if let Some(successor) = successor {
            let destination = position_of(successor.block).ok_or_else(invalid)?;
            if destination == source || preceding[destination].replace(source).is_some() {
                return Err(invalid());
            }
            following[source] = Some(destination);
        }
    }
    if preceding[entry].is_some() {
        return Err(invalid());
    }
    let roots = std::iter::once(entry)
        .chain((0..count).filter(|index| *index != entry && preceding[*index].is_none()));
    let mut visited = vec![false; count];
    let mut order = Vec::with_capacity(count);
    for root in roots {
        let mut current = Some(root);
        while let Some(index) = current {
            if std::mem::replace(&mut visited[index], true) {
                return Err(invalid());
            }
            order.push(&function.blocks[index]);
            current = following[index];
        }
    }
    if order.len() != count {
        return Err(invalid());
    }
    Ok(order)
}

pub(crate) fn layout(
    architecture: Architecture,
    function: &EncodedLayoutFunction,
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    machine_rows: &BTreeMap<SelectedInstructionId, &EncodedAlternativeRequirement>,
    physical: &ValidatedPhysicalRegisterModel,
    policy: SelectedFunctionLayoutPolicy,
    encoded: &SelectedFormEncodedRowStore,
) -> Result<ResolvedSelectedFunctionLayout, Error> {
    let ordered = derive_order(function, policy)?;
    let layout = derive_plan(architecture, &ordered, pre_rows)?;
    let mut blocks = Vec::with_capacity(ordered.len());
    for block in ordered {
        let block_offset = layout.block_offsets[&block.id];
        let mut instruction_offset = block_offset;
        let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
        for instruction in self::instructions(block) {
            let pre = pre_rows
                .get(&instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            let machine = machine_rows
                .get(&instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            if machine.alternative.key != pre.alternative {
                return Err(Error::AlternativeMismatch(instruction.id));
            }
            let (
                bytes,
                branch,
                internal_machine_fixup,
                normalized_foreign_call_fixup,
                image_address_fixup,
            ) = resolve_row(
                architecture,
                block,
                instruction,
                instruction_offset,
                &layout.block_offsets,
                machine,
                pre,
                physical,
                layout.widened_branches.contains(&instruction.id),
                encoded,
            )?;
            let byte_count = u64::try_from(bytes.len()).map_err(|_| Error::OffsetOverflow)?;
            instructions.push(ResolvedSelectedFormRow {
                instruction: instruction.id,
                alternative: pre.alternative,
                offset: instruction_offset,
                bytes,
                branch,
                internal_machine_fixup,
                normalized_foreign_call_fixup,
                image_address_fixup,
            });
            instruction_offset = instruction_offset
                .checked_add(byte_count)
                .ok_or(Error::OffsetOverflow)?;
        }
        let byte_count = layout.block_sizes[&block.id];
        if instruction_offset
            != block_offset
                .checked_add(byte_count)
                .ok_or(Error::OffsetOverflow)?
        {
            return Err(Error::OffsetOverflow);
        }
        blocks.push(ResolvedSelectedBlockLayout {
            block: block.id,
            offset: block_offset,
            byte_count,
            instructions,
        });
    }
    Ok(ResolvedSelectedFunctionLayout {
        machine: function.machine,
        byte_count: layout.function_size,
        blocks,
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_row(
    architecture: Architecture,
    block: &EncodedLayoutBlock,
    instruction: &EncodedLayoutRow,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &EncodedAlternativeRequirement,
    pre: &SelectedFormEncodingRow,
    _physical: &ValidatedPhysicalRegisterModel,
    widened: bool,
    encoded: &SelectedFormEncodedRowStore,
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
                alternatives,
            },
        ) => resolve_branch(
            architecture,
            block,
            instruction,
            instruction_offset,
            block_offsets,
            machine,
            alternatives,
            widened,
        )
        .map(|(bytes, branch)| (bytes, branch, None, None, None)),
        _ => unexpected(instruction.id),
    }
}

fn validate_image_address_fixup(
    instruction: &EncodedLayoutRow,
    bytes: &[u8],
    fixup: SelectedFormImageAddressFixup,
) -> Result<SelectedFormImageAddressFixup, Error> {
    let EncodedLayoutOperation::ImageAddress { reference } = instruction.kind else {
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
    instruction: &EncodedLayoutRow,
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

fn validate_foreign_fixup(
    architecture: Architecture,
    instruction: &EncodedLayoutRow,
    instruction_offset: u64,
    bytes: &[u8],
    fixup: SelectedFormNormalizedForeignCallFixup,
) -> Result<SelectedFormNormalizedForeignCallFixup, Error> {
    let EncodedLayoutOperation::NormalizedForeignCall { boundary, ordinal } = instruction.kind
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

#[allow(clippy::too_many_arguments)]
fn validate_unresolved_field(
    instruction: &EncodedLayoutRow,
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

fn validate_callback_materializations(
    architecture: Architecture,
    instruction: &EncodedLayoutRow,
    bytes: &[u8],
    opcode_row_offset: u16,
    callbacks: &[SelectedFormCallbackMaterialization],
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
                if patch_row_offset as u64 != cursor + 3 || !zero_patch(patch_row_offset)? {
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
                if matches!(
                    callback.destination,
                    CallbackAddressDestination::OutgoingStack { .. }
                ) || page_patch_row_offset as u64 != cursor
                    || page_offset_patch_row_offset as u64 != cursor + 4
                {
                    return unexpected(instruction.id);
                }
                // Encoding owns the address recipe. Layout checks only that
                // both relocation fields are present and still unbound.
                let fields = group
                    .get(..8)
                    .ok_or(Error::UnexpectedEncodingState(instruction.id))?;
                let page =
                    u32::from_le_bytes(fields[..4].try_into().map_err(|_| Error::OffsetOverflow)?);
                let offset =
                    u32::from_le_bytes(fields[4..8].try_into().map_err(|_| Error::OffsetOverflow)?);
                if page & 0x60ff_ffe0 != 0 || offset & 0x003f_fc00 != 0 {
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

#[allow(clippy::too_many_arguments)]
fn resolve_branch(
    architecture: Architecture,
    block: &EncodedLayoutBlock,
    instruction: &EncodedLayoutRow,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &EncodedAlternativeRequirement,
    alternatives: &machine_code::EncodedControlAlternatives,
    widened: bool,
) -> Result<(Vec<u8>, Option<Box<ResolvedBranchEvidence>>), Error> {
    if let EncodedLayoutTerminator::Jump {
        instruction: jump,
        successor,
    } = &block.terminator
    {
        if jump.id != instruction.id {
            return unexpected(instruction.id);
        }
        let target_offset = *block_offsets
            .get(&successor.block)
            .ok_or(Error::BranchFallthroughMismatch(instruction.id))?;
        let encoded = &alternatives.baseline;
        let displacement = match architecture {
            Architecture::X86_64 => byte_displacement(
                target_offset,
                instruction_offset
                    .checked_add(encoded.bytes.len() as u64)
                    .ok_or(Error::OffsetOverflow)?,
            )?,
            Architecture::Aarch64 => byte_displacement(target_offset, instruction_offset)?,
        };
        if encoded.alternative != machine.alternative.key {
            return Err(Error::AlternativeMismatch(instruction.id));
        }
        let bytes = encoded
            .patch(byte_displacement(target_offset, instruction_offset)?)
            .map_err(|_| Error::UnexpectedEncodingState(instruction.id))?;
        let effects = encoded.footprint.encoded.clone();
        if effects != machine.alternative.encoded
            || !declared_size_matches(machine.alternative.size, bytes.len() as u64)
        {
            return unexpected(instruction.id);
        }
        return Ok((
            bytes,
            Some(Box::new(ResolvedBranchEvidence::Jump(
                ResolvedJumpEvidence {
                    source_block: block.id,
                    target_edge: successor.psi_edge,
                    target_block: successor.block,
                    target_offset,
                    byte_displacement: displacement,
                    decoded_effects: effects,
                },
            ))),
        ));
    }
    let Some(descriptor) = conditional_branch_descriptor(block) else {
        return unexpected(instruction.id);
    };
    let ConditionalBranchDescriptor {
        predicate,
        terminator,
        when_taken,
        when_fallthrough,
    } = descriptor;
    if terminator.id != instruction.id {
        return unexpected(instruction.id);
    }
    if machine.alternative.key.family != conditional_branch_family(predicate) {
        return Err(Error::AlternativeMismatch(instruction.id));
    }
    let taken_offset = *block_offsets
        .get(&when_taken.block)
        .ok_or(Error::BranchFallthroughMismatch(instruction.id))?;
    let fallthrough_offset = *block_offsets
        .get(&when_fallthrough.block)
        .ok_or(Error::BranchFallthroughMismatch(instruction.id))?;
    let encoded = if widened {
        alternatives
            .widened
            .as_ref()
            .ok_or(Error::UnexpectedEncodingState(instruction.id))?
    } else {
        &alternatives.baseline
    };
    let byte_count = u64::try_from(encoded.bytes.len()).map_err(|_| Error::OffsetOverflow)?;
    let branch_end = instruction_offset
        .checked_add(byte_count)
        .ok_or(Error::OffsetOverflow)?;
    if fallthrough_offset != branch_end {
        return Err(Error::BranchFallthroughMismatch(instruction.id));
    }
    let displacement = match architecture {
        Architecture::X86_64 => byte_displacement(taken_offset, branch_end)?,
        Architecture::Aarch64 => byte_displacement(taken_offset, instruction_offset)?,
    };

    if encoded.alternative != machine.alternative.key {
        return Err(Error::AlternativeMismatch(instruction.id));
    }
    let bytes = encoded
        .patch(byte_displacement(taken_offset, instruction_offset)?)
        .map_err(|_| Error::UnexpectedEncodingState(instruction.id))?;
    let register_reads = encoded.footprint.register_reads.clone();
    let effects = encoded.footprint.encoded.clone();
    if effects != machine.alternative.encoded {
        return Err(Error::BranchEffectsMismatch(instruction.id));
    }
    if u64::try_from(bytes.len()).ok() != Some(byte_count)
        || !declared_size_matches(machine.alternative.size, byte_count)
    {
        return Err(Error::BranchSizeMismatch(instruction.id));
    }
    Ok((
        bytes,
        Some(Box::new(ResolvedBranchEvidence::Conditional(
            ResolvedConditionalBranchEvidence {
                predicate,
                source_block: block.id,
                when_taken_edge: when_taken.psi_edge,
                when_taken_block: when_taken.block,
                when_taken_offset: taken_offset,
                when_fallthrough_edge: when_fallthrough.psi_edge,
                when_fallthrough_block: when_fallthrough.block,
                when_fallthrough_offset: fallthrough_offset,
                byte_displacement: displacement,
                decoded_register_reads: register_reads,
                decoded_effects: effects,
            },
        ))),
    ))
}

#[cfg(test)]
mod tests {
    fn derive_order(
        function: &selected_instructions::SelectedFunction,
        policy: crate::SelectedFunctionLayoutPolicy,
    ) -> Result<
        Vec<&selected_instructions::SelectedBlock>,
        crate::OptimizedResolvedSelectedFormLayoutError,
    > {
        crate::order_suite::projected_order(function, policy, super::derive_order)
    }
    crate::order_suite::order_suite!(derive_order);
}
