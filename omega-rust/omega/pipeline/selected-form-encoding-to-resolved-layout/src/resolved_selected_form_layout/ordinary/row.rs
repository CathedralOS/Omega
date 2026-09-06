use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use post_allocation_machine_to_post_allocation_machine::{
    Aarch64CbnzFusionAction, Aarch64SameViewCopyElisionAction,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use semantic_vocabulary::MachineId;
use target::Architecture;

use post_allocation_machine_to_post_allocation_machine::{
    StagedOptimizedAarch64CbnzFusion, StagedOptimizedAarch64SameViewCopyElision,
};
use post_allocation_machine_to_selected_form_encoding::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormMachineDisposition,
};

use super::super::{OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence};
use super::branch;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    architecture: Architecture,
    function: MachineId,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    instruction_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
    copy_elision: Option<&StagedOptimizedAarch64SameViewCopyElision>,
) -> Result<
    (
        Vec<u8>,
        Option<Box<ResolvedBranchEvidence>>,
        Option<SelectedFormInternalMachineFixup>,
    ),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (&pre.machine_disposition, &pre.state) {
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => Ok((bytes.clone(), None, None)),
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
                *fixup,
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
            None,
        )
        .map(|(bytes, branch)| (bytes, branch, None)),
        (
            SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { consumer },
            SelectedFormEncodingState::Encoded { .. },
        ) => {
            let action = fusion_action(fusion, function, block.id, instruction.id, *consumer)?;
            if architecture != Architecture::Aarch64
                || !matches!(instruction.kind, SelectedInstructionKind::CompareI64Zero)
                || action.compare != instruction.id
                || action.branch != *consumer
            {
                return unexpected(instruction.id);
            }
            Ok((Vec::new(), None, None))
        }
        (
            SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 {
                compare,
                source_read,
            },
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => {
            let action = fusion_action(fusion, function, block.id, *compare, instruction.id)?;
            if architecture != Architecture::Aarch64 || &action.source_read != source_read {
                return unexpected(instruction.id);
            }
            branch::resolve(
                architecture,
                block,
                instruction,
                instruction_offset,
                block_offsets,
                machine,
                physical,
                Some((source_read, action)),
            )
            .map(|(bytes, branch)| (bytes, branch, None))
        }
        (
            SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { consumer },
            SelectedFormEncodingState::Encoded { .. },
        ) => {
            let action = copy_action(copy_elision, function, block.id, instruction.id, *consumer)?;
            if architecture != Architecture::Aarch64
                || !matches!(instruction.kind, SelectedInstructionKind::CopyI64)
                || action.copy != instruction.id
                || action.consumer != *consumer
            {
                return unexpected(instruction.id);
            }
            Ok((Vec::new(), None, None))
        }
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
    let SelectedInstructionKind::CallI64 { callee } = instruction.kind else {
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

fn copy_action(
    elision: Option<&StagedOptimizedAarch64SameViewCopyElision>,
    machine: MachineId,
    block: SelectedBlockId,
    copy: SelectedInstructionId,
    returned: SelectedInstructionId,
) -> Result<&Aarch64SameViewCopyElisionAction, OptimizedResolvedSelectedFormLayoutError> {
    elision
        .and_then(|elision| {
            elision.elision().plan().actions.iter().find(|action| {
                action.machine == machine
                    && action.block == block
                    && action.copy == copy
                    && action.consumer == returned
            })
        })
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(copy))
}

fn fusion_action(
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
    machine: MachineId,
    block: SelectedBlockId,
    compare: SelectedInstructionId,
    branch: SelectedInstructionId,
) -> Result<&Aarch64CbnzFusionAction, OptimizedResolvedSelectedFormLayoutError> {
    fusion
        .and_then(|fusion| {
            fusion.fusion().plan().actions.iter().find(|action| {
                action.machine == machine
                    && action.block == block
                    && action.compare == compare
                    && action.branch == branch
            })
        })
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(branch))
}

fn unexpected<T>(
    instruction: SelectedInstructionId,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction))
}
