use std::collections::BTreeMap;

use omega_machine_optimizer::{
    Aarch64CbnzFusionAction, Aarch64CbnzInstructionDisposition, PostAllocationMachineInstruction,
};
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionId,
};
use omega_target::Architecture;
use psi_core::MachineId;

use crate::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    StagedOptimizedAarch64CbnzFusion,
};

use super::super::{OptimizedResolvedSelectedFormLayoutError, ResolvedSelectedFormRow};
use super::branch;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    architecture: Architecture,
    function: MachineId,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
    fusion: Option<&StagedOptimizedAarch64CbnzFusion>,
    expected_offset: u64,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
    candidate: &ResolvedSelectedFormRow,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    if candidate.instruction != instruction.id
        || candidate.alternative != pre.alternative
        || candidate.alternative != machine.alternative.key
        || candidate.offset != expected_offset
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    match (&pre.machine_disposition, &pre.state) {
        (
            Aarch64CbnzInstructionDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => {
            if candidate.bytes != *bytes || candidate.branch.is_some() {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
        }
        (
            Aarch64CbnzInstructionDisposition::RetainedV1,
            SelectedFormEncodingState::DeferredControl {
                reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
            },
        ) => branch::validate(
            architecture,
            block,
            instruction,
            expected_offset,
            block_offsets,
            machine,
            physical,
            None,
            candidate,
        ),
        (
            Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 { consumer },
            SelectedFormEncodingState::Encoded { .. },
        ) => {
            let action = fusion_action(fusion, function, block.id, instruction.id, *consumer)?;
            if architecture != Architecture::Aarch64
                || !matches!(
                    instruction.kind,
                    omega_selected_instructions::SelectedInstructionKind::CompareI64Zero
                )
                || action.compare != instruction.id
                || action.branch != *consumer
                || !candidate.bytes.is_empty()
                || candidate.branch.is_some()
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
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
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            branch::validate(
                architecture,
                block,
                instruction,
                expected_offset,
                block_offsets,
                machine,
                physical,
                Some((source_read, action)),
                candidate,
            )
        }
        _ => Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch),
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
        .ok_or(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch)
}
