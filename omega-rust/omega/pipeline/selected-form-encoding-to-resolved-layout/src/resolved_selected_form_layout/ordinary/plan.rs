use std::collections::BTreeMap;

use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedTerminator,
};
use target::Architecture;

use post_allocation_machine_to_selected_form_encoding::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};

use super::super::OptimizedResolvedSelectedFormLayoutError;

pub(super) struct LayoutPlan {
    pub(super) block_offsets: BTreeMap<SelectedBlockId, u64>,
    pub(super) block_sizes: BTreeMap<SelectedBlockId, u64>,
    pub(super) function_size: u64,
}

pub(super) fn derive(
    architecture: Architecture,
    blocks: &[&SelectedBlock],
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
) -> Result<LayoutPlan, OptimizedResolvedSelectedFormLayoutError> {
    let mut block_offsets = BTreeMap::new();
    let mut block_sizes = BTreeMap::new();
    let mut offset = 0_u64;
    for block in blocks {
        block_offsets.insert(block.id, offset);
        let start = offset;
        for instruction in instructions(block) {
            let pre = pre_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            offset = offset
                .checked_add(instruction_size(architecture, instruction, pre)?)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        }
        block_sizes.insert(block.id, offset - start);
    }
    Ok(LayoutPlan {
        block_offsets,
        block_sizes,
        function_size: offset,
    })
}

pub(in super::super) fn instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}

fn instruction_size(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    row: &SelectedFormEncodingRow,
) -> Result<u64, OptimizedResolvedSelectedFormLayoutError> {
    match &row.machine_disposition {
        SelectedFormMachineDisposition::Aarch64ElidedCompareI64ZeroV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(instruction.kind, SelectedInstructionKind::CompareI64Zero)
                && matches!(row.state, SelectedFormEncodingState::Encoded { .. })
            {
                return Ok(0);
            }
            return unexpected(instruction);
        }
        SelectedFormMachineDisposition::Aarch64FusedBranchNonZeroToCbnzV1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(row.state, SelectedFormEncodingState::DeferredControl { .. })
            {
                return Ok(branch_size(architecture));
            }
            return unexpected(instruction);
        }
        SelectedFormMachineDisposition::Aarch64ElidedSameViewCopyI64V1 { .. } => {
            if architecture == Architecture::Aarch64
                && matches!(instruction.kind, SelectedInstructionKind::CopyI64)
                && matches!(row.state, SelectedFormEncodingState::Encoded { .. })
            {
                return Ok(0);
            }
            return unexpected(instruction);
        }
        SelectedFormMachineDisposition::RetainedV1 => {}
    }
    match &row.state {
        SelectedFormEncodingState::Encoded { bytes, .. } => u64::try_from(bytes.len())
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow),
        SelectedFormEncodingState::UnresolvedInternalMachineCall { bytes, .. } => {
            if !matches!(instruction.kind, SelectedInstructionKind::CallI64 { .. }) {
                return unexpected(instruction);
            }
            u64::try_from(bytes.len())
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)
        }
        SelectedFormEncodingState::DeferredControl {
            reason: DeferredControlEncodingReason::RequiresResolvedBranchLayout,
        } => {
            if row.instruction != instruction.id {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                );
            }
            Ok(
                if matches!(instruction.kind, SelectedInstructionKind::Jump) {
                    match architecture {
                        Architecture::X86_64 => 5,
                        Architecture::Aarch64 => 4,
                    }
                } else {
                    branch_size(architecture)
                },
            )
        }
    }
}

const fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

fn unexpected<T>(
    instruction: &SelectedInstruction,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction.id))
}
