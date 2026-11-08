/*
We construct function-relative block and instruction layouts for ordinary functions in stage 08. We select the program-wide layout policy (`select`), order blocks along mandatory conditional fallthrough chains while keeping storage positions independent of sparse block identifiers (`derive_order`), iteratively widen AArch64 conditional branches whose taken displacement exceeds the signed 19-bit word range (`derive_plan`), and resolve each row's byte vector, branch evidence, and call or image-address fixup bounds (`layout`).
*/

use std::collections::{BTreeMap, BTreeSet};

use machine_code::{
    CallbackAddressDestination, DeferredControlEncodingReason, ImageAddressFields,
    SelectedFormCallbackMaterialization, SelectedFormCallbackMaterializationEncoding,
    SelectedFormEncodedRowStore, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormImageAddressFixup, SelectedFormInternalMachineFixup,
    SelectedFormInternalMachineFixupKind, SelectedFormInternalMachineFixupState,
    SelectedFormMachineDisposition, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};
use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily, SelectedBlock, SelectedBlockId, SelectedFunction,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedSuccessor, SelectedTerminator,
};
use target::Architecture;

use crate::{
    OptimizedResolvedSelectedFormLayoutError as Error, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedJumpEvidence,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    SelectedFunctionLayoutPolicy, declared_size_matches, is_admitted_canonical_shape,
    is_single_entry,
};

pub(crate) struct ConditionalBranchDescriptor<'a> {
    pub(crate) predicate: ResolvedConditionalBranchPredicate, // Branch predicate family encoded by the terminator.
    pub(crate) terminator: &'a SelectedInstruction, // Underlying conditional-branch instruction row.
    pub(crate) when_taken: &'a SelectedSuccessor, // Successor edge reached when the predicate holds.
    pub(crate) when_fallthrough: &'a SelectedSuccessor, // Successor edge that must sit immediately after the branch.
}

pub(crate) struct LayoutPlan {
    pub(crate) block_offsets: BTreeMap<SelectedBlockId, u64>, // Function-relative start offset of each block.
    pub(crate) block_sizes: BTreeMap<SelectedBlockId, u64>,   // Total byte size of each block.
    pub(crate) instruction_offsets: BTreeMap<SelectedInstructionId, u64>, // Function-relative byte offset of each instruction.
    pub(crate) function_size: u64, // Total byte size of the function across all blocks.
    pub(crate) widened_branches: BTreeSet<SelectedInstructionId>, // AArch64 conditional branches widened to 8-byte skip+branch pairs.
}

pub(crate) fn select(
    selected: &SelectedInstructionPlan,
) -> Result<SelectedFunctionLayoutPolicy, Error> {
    let single_entry_count = selected
        .functions
        .iter()
        .filter(|function| is_single_entry(function))
        .count();
    if single_entry_count == selected.functions.len() {
        Ok(SelectedFunctionLayoutPolicy::SingleEntryBlockV1)
    } else if single_entry_count == 0
        && selected.functions.iter().all(|function| {
            matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(SelectedTerminator::ConditionalBranch { .. })
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1)
    } else if single_entry_count == 0
        && selected.functions.iter().all(|function| {
            matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(selected_instructions::conditional_branch_less_pair_pattern!(..))
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1)
    } else if selected.functions.iter().all(is_admitted_canonical_shape) {
        Ok(SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1)
    } else {
        Err(Error::UnsupportedFunctionShape(
            selected.functions[single_entry_count].machine,
        ))
    }
}

pub(crate) fn derive_order(
    function: &SelectedFunction,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&SelectedBlock>, Error> {
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
                SelectedTerminator::Return { .. }
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
    function: &SelectedFunction,
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    machine_rows: &BTreeMap<SelectedInstructionId, &PostAllocationMachineInstruction>,
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

pub(crate) fn derive_plan(
    architecture: Architecture,
    blocks: &[&SelectedBlock],
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
) -> Result<LayoutPlan, Error> {
    let mut widened = BTreeSet::new();
    loop {
        let plan = derive_once(architecture, blocks, pre_rows, &widened)?;
        if architecture != Architecture::Aarch64 {
            return Ok(plan);
        }
        let mut grown = widened;
        for block in blocks {
            let Some(descriptor) = conditional_branch_descriptor(block) else {
                continue;
            };
            if grown.contains(&descriptor.terminator.id) {
                continue;
            }
            let missing = || Error::MissingInstruction(descriptor.terminator.id);
            let row = pre_rows
                .get(&descriptor.terminator.id)
                .ok_or_else(missing)?;
            if !widening_candidate(descriptor.terminator, row) {
                continue;
            }
            let taken_offset = *plan
                .block_offsets
                .get(&descriptor.when_taken.block)
                .ok_or_else(missing)?;
            let instruction_offset = *plan
                .instruction_offsets
                .get(&descriptor.terminator.id)
                .ok_or_else(missing)?;
            let displacement = byte_displacement(taken_offset, instruction_offset)?;
            if !isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&displacement) {
                grown.insert(descriptor.terminator.id);
            }
        }
        if grown == plan.widened_branches {
            return Ok(plan);
        }
        widened = grown;
    }
}

fn derive_once(
    architecture: Architecture,
    blocks: &[&SelectedBlock],
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
    widened: &BTreeSet<SelectedInstructionId>,
) -> Result<LayoutPlan, Error> {
    let mut block_offsets = BTreeMap::new();
    let mut block_sizes = BTreeMap::new();
    let mut instruction_offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in blocks {
        block_offsets.insert(block.id, offset);
        let start = offset;
        for instruction in instructions(block) {
            instruction_offsets.insert(instruction.id, offset);
            let pre = pre_rows
                .get(&instruction.id)
                .ok_or(Error::MissingInstruction(instruction.id))?;
            offset = offset
                .checked_add(instruction_size(architecture, instruction, pre, widened)?)
                .ok_or(Error::OffsetOverflow)?;
        }
        block_sizes.insert(block.id, offset - start);
    }
    Ok(LayoutPlan {
        block_offsets,
        block_sizes,
        instruction_offsets,
        function_size: offset,
        widened_branches: widened.clone(),
    })
}

fn widening_candidate(instruction: &SelectedInstruction, row: &SelectedFormEncodingRow) -> bool {
    matches!(
        instruction.kind,
        SelectedInstructionKind::ConditionalBranchNonZero
            | SelectedInstructionKind::ConditionalBranchU64LessThan
            | SelectedInstructionKind::ConditionalBranchI64LessThan
    ) && matches!(
        row.machine_disposition,
        SelectedFormMachineDisposition::RetainedV1
    ) && matches!(
        row.state,
        SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
        }
    )
}

pub(crate) fn byte_displacement(target: u64, base: u64) -> Result<i64, Error> {
    i64::try_from(i128::from(target) - i128::from(base)).map_err(|_| Error::OffsetOverflow)
}

pub(crate) fn instructions(block: &SelectedBlock) -> impl Iterator<Item = &SelectedInstruction> {
    block.instructions.iter().chain(std::iter::once(
        selected_instructions::terminator_instruction(&block.terminator),
    ))
}

fn instruction_size(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    row: &SelectedFormEncodingRow,
    widened: &BTreeSet<SelectedInstructionId>,
) -> Result<u64, Error> {
    match &row.machine_disposition {
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(instruction.kind, SelectedInstructionKind::CompareI64Zero)
                && matches!(row.state, SelectedFormEncodingState::Encoded)
            {
                return Ok(0);
            }
            return unexpected(instruction.id);
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(row.state, SelectedFormEncodingState::DeferredControl { .. })
            {
                return Ok(branch_size(architecture));
            }
            return unexpected(instruction.id);
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(instruction.kind, SelectedInstructionKind::CopyI64)
                && matches!(row.state, SelectedFormEncodingState::Encoded)
            {
                return Ok(0);
            }
            return unexpected(instruction.id);
        }
        SelectedFormMachineDisposition::RetainedV1 => {}
    }
    match &row.state {
        SelectedFormEncodingState::Encoded => Ok(row.code.byte_count),
        SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
            if !matches!(
                instruction.kind,
                selected_instructions::selected_call_pattern!(..)
            ) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
        }
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { .. } => {
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::NormalizedForeignCall { .. }
            ) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
        }
        SelectedFormEncodingState::UnresolvedImageAddress { .. } => {
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::ImageAddress { .. }
            ) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
        }
        SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
        } => {
            if row.instruction != instruction.id {
                return Err(Error::MissingInstruction(instruction.id));
            }
            Ok(
                if matches!(instruction.kind, SelectedInstructionKind::Jump) {
                    match architecture {
                        Architecture::X86_64 => 5,
                        Architecture::Aarch64 => 4,
                    }
                } else if widened.contains(&instruction.id) {
                    8
                } else {
                    branch_size(architecture)
                },
            )
        }
    }
}

pub(crate) const fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

pub(crate) fn unexpected<T>(instruction: SelectedInstructionId) -> Result<T, Error> {
    Err(Error::UnexpectedEncodingState(instruction))
}

#[allow(clippy::too_many_arguments)]
fn resolve_row(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
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
            },
        ) => resolve_branch(
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

fn validate_callback_materializations(
    architecture: Architecture,
    instruction: &SelectedInstruction,
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

pub(crate) fn conditional_branch_descriptor(
    block: &SelectedBlock,
) -> Option<ConditionalBranchDescriptor<'_>> {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::NonZeroV1,
            terminator: instruction,
            when_taken: when_nonzero,
            when_fallthrough: when_zero,
        }),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::U64LessThanV1,
            terminator: instruction,
            when_taken: when_less,
            when_fallthrough: when_not_less,
        }),
        SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            when_not_less,
        } => Some(ConditionalBranchDescriptor {
            predicate: ResolvedConditionalBranchPredicate::I64LessThanV1,
            terminator: instruction,
            when_taken: when_less,
            when_fallthrough: when_not_less,
        }),
        SelectedTerminator::Jump { .. }
        | selected_instructions::terminator_no_successor_pattern!(..) => None,
    }
}

pub(crate) const fn conditional_branch_family(
    predicate: ResolvedConditionalBranchPredicate,
) -> MachineAlternativeFamily {
    match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => {
            MachineAlternativeFamily::ConditionalBranchNonZero
        }
        ResolvedConditionalBranchPredicate::U64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchU64LessThan
        }
        ResolvedConditionalBranchPredicate::I64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchI64LessThan
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_branch(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    widened: bool,
) -> Result<(Vec<u8>, Option<Box<ResolvedBranchEvidence>>), Error> {
    if let SelectedTerminator::Jump {
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
        let (bytes, displacement, effects) = match architecture {
            Architecture::X86_64 => {
                let end = instruction_offset
                    .checked_add(5)
                    .ok_or(Error::OffsetOverflow)?;
                let displacement = byte_displacement(target_offset, end)?;
                let result = isa_x86_64::encode_x86_64_selected_jump_form(
                    physical,
                    machine.alternative.key,
                    displacement,
                )
                .map_err(Error::X86_64)?;
                (
                    result.bytes().to_vec(),
                    displacement,
                    result.footprint().encoded.clone(),
                )
            }
            Architecture::Aarch64 => {
                let displacement = byte_displacement(target_offset, instruction_offset)?;
                let result = isa_aarch64::encode_aarch64_selected_jump_form(
                    physical,
                    machine.alternative.key,
                    displacement,
                )
                .map_err(Error::Aarch64)?;
                (
                    result.bytes().to_vec(),
                    displacement,
                    result.footprint().encoded.clone(),
                )
            }
        };
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
    let byte_count = if widened {
        8
    } else {
        branch_size(architecture)
    };
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
    let (bytes, register_reads, effects) = match architecture {
        Architecture::X86_64 => {
            let realized = match predicate {
                ResolvedConditionalBranchPredicate::NonZeroV1 => {
                    isa_x86_64::encode_x86_64_selected_nonzero_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                }
                ResolvedConditionalBranchPredicate::U64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                }
                ResolvedConditionalBranchPredicate::I64LessThanV1 => {
                    isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                        physical,
                        machine.alternative.key,
                        displacement,
                    )
                }
            }
            .map_err(Error::X86_64)?;
            (
                realized.bytes().to_vec(),
                realized.footprint().register_reads.clone(),
                realized.footprint().encoded.clone(),
            )
        }
        Architecture::Aarch64 => {
            let form = if widened {
                isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1
            } else {
                isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1
            };
            let realized = isa_aarch64::encode_aarch64_selected_conditional_branch_form(
                physical,
                machine.alternative.key,
                conditional_branch_family(predicate),
                form,
                displacement,
            )
            .map_err(Error::Aarch64)?;
            (
                realized.bytes().to_vec(),
                realized.footprint().register_reads.clone(),
                realized.footprint().encoded.clone(),
            )
        }
    };
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
    crate::order_suite::order_suite!(super::derive_order);
}
