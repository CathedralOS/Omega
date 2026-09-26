use std::collections::{BTreeMap, BTreeSet};

use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
};
use target::Architecture;

use machine_code::{
    DeferredControlEncodingReason, SelectedFormEncodingRow, SelectedFormEncodingState,
    SelectedFormMachineDisposition,
};

use super::super::OptimizedResolvedSelectedFormLayoutError;

pub(super) struct LayoutPlan {
    pub(super) block_offsets: BTreeMap<SelectedBlockId, u64>,
    pub(super) block_sizes: BTreeMap<SelectedBlockId, u64>,
    pub(super) instruction_offsets: BTreeMap<SelectedInstructionId, u64>,
    pub(super) function_size: u64,
    /// AArch64 conditional branches realized as `B.<invcond> +8; B target`
    /// because the taken edge left the imm19 range under the final offsets.
    pub(super) widened_branches: BTreeSet<SelectedInstructionId>,
}

pub(super) fn derive(
    architecture: Architecture,
    blocks: &[&SelectedBlock],
    pre_rows: &BTreeMap<SelectedInstructionId, &SelectedFormEncodingRow>,
) -> Result<LayoutPlan, OptimizedResolvedSelectedFormLayoutError> {
    // AArch64 conditional branches widen by four bytes when the taken edge
    // leaves the `B.cond` imm19 range; widening grows later offsets, so the
    // widened set is monotone across iterations and this loop converges.
    let mut widened = BTreeSet::new();
    loop {
        let plan = derive_once(architecture, blocks, pre_rows, &widened)?;
        if architecture != Architecture::Aarch64 {
            return Ok(plan);
        }
        let mut grown = widened;
        for block in blocks {
            let Some((branch, taken)) = conditional_branch(block) else {
                continue;
            };
            if grown.contains(&branch.id) {
                continue;
            }
            let row = pre_rows
                .get(&branch.id)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(branch.id))?;
            if !widening_candidate(branch, row) {
                continue;
            }
            let taken_offset = *plan
                .block_offsets
                .get(&taken.block)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(branch.id))?;
            let instruction_offset = *plan
                .instruction_offsets
                .get(&branch.id)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::MissingInstruction(branch.id))?;
            let displacement = byte_displacement(taken_offset, instruction_offset)?;
            if aarch64_branch_overflows_imm19(displacement) {
                grown.insert(branch.id);
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
) -> Result<LayoutPlan, OptimizedResolvedSelectedFormLayoutError> {
    let mut block_offsets = BTreeMap::new();
    let mut block_sizes = BTreeMap::new();
    let mut instruction_offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in blocks {
        block_offsets.insert(block.id, offset);
        let start = offset;
        for instruction in instructions(block) {
            instruction_offsets.insert(instruction.id, offset);
            let pre = pre_rows.get(&instruction.id).ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            offset = offset
                .checked_add(instruction_size(architecture, instruction, pre, widened)?)
                .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
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

fn conditional_branch(block: &SelectedBlock) -> Option<(&SelectedInstruction, &SelectedSuccessor)> {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            ..
        } => Some((instruction, when_nonzero)),
        SelectedTerminator::ConditionalBranchU64LessThan {
            instruction,
            when_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            instruction,
            when_less,
            ..
        } => Some((instruction, when_less)),
        SelectedTerminator::Jump { .. }
        | SelectedTerminator::Return { .. }
        | SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. } => None,
    }
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

fn aarch64_branch_overflows_imm19(byte_displacement: i64) -> bool {
    byte_displacement % 4 == 0
        && !(-(1_i64 << 18)..(1_i64 << 18)).contains(&(byte_displacement / 4))
}

fn byte_displacement(
    target: u64,
    base: u64,
) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)
}

pub(in super::super) fn instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. }
            | SelectedTerminator::Crash { instruction, .. }
            | SelectedTerminator::HostedExitProcess { instruction, .. } => instruction,
        }))
}

fn instruction_size(
    architecture: Architecture,
    instruction: &SelectedInstruction,
    row: &SelectedFormEncodingRow,
    widened: &BTreeSet<SelectedInstructionId>,
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
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::CallScalar { .. }
                    | SelectedInstructionKind::CallUnit { .. }
                    | SelectedInstructionKind::CallAggregate { .. }
            ) {
                return unexpected(instruction);
            }
            u64::try_from(bytes.len())
                .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)
        }
        SelectedFormEncodingState::UnresolvedNormalizedForeignCall { bytes, .. } => {
            if !matches!(
                instruction.kind,
                SelectedInstructionKind::NormalizedForeignCall { .. }
            ) {
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
                } else if widened.contains(&instruction.id) {
                    8
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
