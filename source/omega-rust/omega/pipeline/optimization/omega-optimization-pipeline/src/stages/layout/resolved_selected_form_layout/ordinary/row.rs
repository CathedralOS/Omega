use std::collections::BTreeMap;

use omega_machine_optimizer::{
    Aarch64CbnzFusionAction, Aarch64SameViewCopyElisionAction, PostAllocationMachineInstruction,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind,
};
use omega_target::Architecture;
use psi_core::MachineId;

use crate::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition, StagedOptimizedAarch64CbnzFusion,
    StagedOptimizedAarch64SameViewCopyElision,
};

use super::super::{OptimizedResolvedSelectedFormLayoutError, ResolvedConditionalBranchEvidence};
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
    (Vec<u8>, Option<Box<ResolvedConditionalBranchEvidence>>),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (&pre.machine_disposition, &pre.state) {
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => Ok((bytes.clone(), None)),
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
        ),
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
            Ok((Vec::new(), None))
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
        }
        (
            SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { consumer },
            SelectedFormEncodingState::Encoded { .. },
        ) => {
            let action = copy_action(copy_elision, function, block.id, instruction.id, *consumer)?;
            if architecture != Architecture::Aarch64
                || !matches!(instruction.kind, SelectedInstructionKind::CopyI64)
                || action.copy != instruction.id
                || action.return_instruction != *consumer
            {
                return unexpected(instruction.id);
            }
            Ok((Vec::new(), None))
        }
        _ => unexpected(instruction.id),
    }
}

fn copy_action<'a>(
    elision: Option<&'a StagedOptimizedAarch64SameViewCopyElision>,
    machine: MachineId,
    block: SelectedBlockId,
    copy: SelectedInstructionId,
    returned: SelectedInstructionId,
) -> Result<&'a Aarch64SameViewCopyElisionAction, OptimizedResolvedSelectedFormLayoutError> {
    elision
        .and_then(|elision| {
            elision.elision().plan().actions.iter().find(|action| {
                action.machine == machine
                    && action.block == block
                    && action.copy == copy
                    && action.return_instruction == returned
            })
        })
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(copy))
}

fn fusion_action<'a>(
    fusion: Option<&'a StagedOptimizedAarch64CbnzFusion>,
    machine: MachineId,
    block: SelectedBlockId,
    compare: SelectedInstructionId,
    branch: SelectedInstructionId,
) -> Result<&'a Aarch64CbnzFusionAction, OptimizedResolvedSelectedFormLayoutError> {
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
