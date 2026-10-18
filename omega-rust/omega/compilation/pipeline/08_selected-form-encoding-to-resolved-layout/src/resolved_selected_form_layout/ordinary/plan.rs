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

pub(in super::super) struct LayoutPlan {
    pub(in super::super) block_offsets: BTreeMap<SelectedBlockId, u64>,
    pub(in super::super) block_sizes: BTreeMap<SelectedBlockId, u64>,
    pub(in super::super) instruction_offsets: BTreeMap<SelectedInstructionId, u64>,
    pub(in super::super) function_size: u64,
    /// AArch64 conditional branches realized as `B.<invcond> +8; B target`
    /// because the taken edge left the imm19 range under the final offsets.
    pub(in super::super) widened_branches: BTreeSet<SelectedInstructionId>,
}

pub(in super::super) fn derive(
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
            if !isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&displacement) {
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

pub(crate) fn conditional_branch(
    block: &SelectedBlock,
) -> Option<(&SelectedInstruction, &SelectedSuccessor)> {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            ..
        } => Some((instruction, when_nonzero)),
        selected_instructions::conditional_branch_less_pair_pattern!(
            instruction,
            when_less,
            ..
        ) => Some((instruction, when_less)),
        SelectedTerminator::Jump { .. }
        | selected_instructions::terminator_exit_triple_pattern!(..) => None,
    }
}

pub(crate) fn widening_candidate(
    instruction: &SelectedInstruction,
    row: &SelectedFormEncodingRow,
) -> bool {
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

pub(crate) fn byte_displacement(
    target: u64,
    base: u64,
) -> Result<i64, OptimizedResolvedSelectedFormLayoutError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)
}

pub(in super::super) fn instructions(
    block: &SelectedBlock,
) -> impl Iterator<Item = &SelectedInstruction> {
    block.instructions.iter().chain(std::iter::once(
        selected_instructions::terminator_instruction(&block.terminator),
    ))
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
    // An encoded row's size is its interned byte span's count; no row owns a
    // byte vector whose length has to be narrowed.
    match &row.state {
        SelectedFormEncodingState::Encoded => Ok(row.code.byte_count),
        SelectedFormEncodingState::UnresolvedInternalMachineCall { .. } => {
            if !matches!(
                instruction.kind,
                selected_instructions::selected_call_triple_pattern!(..)
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
                SelectedInstructionKind::ImmutableImageAddress { .. }
            ) {
                return unexpected(instruction.id);
            }
            Ok(row.code.byte_count)
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

pub(crate) const fn branch_size(architecture: Architecture) -> u64 {
    match architecture {
        Architecture::X86_64 => 6,
        Architecture::Aarch64 => 4,
    }
}

pub(crate) fn unexpected<T>(
    instruction: SelectedInstructionId,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnexpectedEncodingState(instruction))
}
