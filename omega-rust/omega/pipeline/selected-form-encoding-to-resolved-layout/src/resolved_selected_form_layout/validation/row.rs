use std::collections::BTreeMap;

use physical_instructions::PostAllocationMachineInstruction;
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionKind,
};
use target::Architecture;

use machine_code::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};

use super::super::{OptimizedResolvedSelectedFormLayoutError, ResolvedSelectedFormRow};
use super::branch;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    architecture: Architecture,
    block: &SelectedBlock,
    instruction: &SelectedInstruction,
    machine: &PostAllocationMachineInstruction,
    pre: &SelectedFormEncodingRow,
    physical: &ValidatedPhysicalRegisterModel,
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
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::Encoded { bytes, .. },
        ) => {
            if candidate.bytes != *bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedInternalMachineCall { bytes, fixup, .. },
        ) => {
            if !matches!(instruction.kind, SelectedInstructionKind::CallI64 { callee } | SelectedInstructionKind::CallUnit { callee } if callee == fixup.callee)
                || candidate.bytes != *bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup != Some(*fixup)
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
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
            candidate,
        ),
        _ => Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch),
    }
}
