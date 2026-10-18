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
    widened: bool,
    encoded: &machine_code::SelectedFormEncodedRowStore,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    if candidate.instruction != instruction.id
        || candidate.alternative != pre.alternative
        || candidate.alternative != machine.alternative.key
        || candidate.offset != expected_offset
    {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    // The layout row restates the encoding row's interned bytes; the span
    // resolves them once for every disposition below.
    let bytes = encoded.bytes(pre.code);
    match (&pre.machine_disposition, &pre.state) {
        (SelectedFormMachineDisposition::RetainedV1, SelectedFormEncodingState::Encoded) => {
            if candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
                || candidate.normalized_foreign_call_fixup.is_some()
                || candidate.image_address_fixup.is_some()
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedInternalMachineCall { fixup },
        ) => {
            if !matches!(instruction.kind, selected_instructions::selected_call_triple_pattern!(target:selected_instructions::SelectedCallTarget::Machine(callee), ..) if callee == fixup.callee)
                || candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup != Some(*fixup)
                || candidate.normalized_foreign_call_fixup.is_some()
                || candidate.image_address_fixup.is_some()
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedNormalizedForeignCall { fixup },
        ) => {
            if !matches!(instruction.kind, SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } if boundary == fixup.boundary && ordinal == fixup.ordinal)
                || candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
                || candidate.normalized_foreign_call_fixup.as_ref() != Some(fixup)
                || candidate.image_address_fixup.is_some()
            {
                return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
            }
            Ok(())
        }
        (
            SelectedFormMachineDisposition::RetainedV1,
            SelectedFormEncodingState::UnresolvedImageAddress { fixup },
        ) => {
            if !matches!(instruction.kind, SelectedInstructionKind::ImmutableImageAddress { reference } if fixup.reference == reference)
                || candidate.bytes != bytes
                || candidate.branch.is_some()
                || candidate.internal_machine_fixup.is_some()
                || candidate.normalized_foreign_call_fixup.is_some()
                || candidate.image_address_fixup != Some(*fixup)
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
            widened,
        ),
        _ => Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch),
    }
}
