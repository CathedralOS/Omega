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
        )
        .map(|(bytes, branch)| (bytes, branch, None)),
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
    let (SelectedInstructionKind::CallI64 { callee }
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

fn unexpected<T>(
    instruction: SelectedInstructionId,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction))
}
