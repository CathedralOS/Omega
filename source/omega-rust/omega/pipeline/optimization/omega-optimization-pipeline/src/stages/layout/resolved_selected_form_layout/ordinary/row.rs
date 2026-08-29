use std::collections::BTreeMap;

use omega_machine_optimizer::{
    Aarch64CbnzFusionAction, Aarch64CbnzInstructionDisposition, PostAllocationMachineInstruction,
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
    StagedOptimizedAarch64CbnzFusion,
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
) -> Result<
    (Vec<u8>, Option<Box<ResolvedConditionalBranchEvidence>>),
    OptimizedResolvedSelectedFormLayoutError,
> {
    match (&pre.machine_disposition, &pre.state) {
        (
            Aarch64CbnzInstructionDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => Ok((bytes.clone(), None)),
        (
            Aarch64CbnzInstructionDisposition::RetainedV1,
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
            Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer },
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
            Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
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
        _ => unexpected(instruction.id),
    }
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
