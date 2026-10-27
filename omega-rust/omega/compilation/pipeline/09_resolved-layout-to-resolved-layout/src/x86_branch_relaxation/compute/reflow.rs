//! Dense offset reconstruction and branch re-encoding.

use std::collections::BTreeMap;

use isa_x86_64::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};

use machine_code::{ResolvedConditionalBranchPredicate, ResolvedSelectedFunctionLayout};

use super::super::error::OptimizedX86BranchRelaxationError;
use super::work::checked_delta;

pub(super) fn reflow_production_functions(
    functions: &mut [ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let offsets = assign_dense_offsets(function)?;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_deref_mut() else {
                    continue;
                };
                let branch = match branch {
                    machine_code::ResolvedBranchEvidence::Conditional(branch) => branch,
                    machine_code::ResolvedBranchEvidence::Jump(jump) => {
                        let target = *offsets.get(&jump.target_block).ok_or(
                            OptimizedX86BranchRelaxationError::MissingTargetBlock(
                                jump.target_block,
                            ),
                        )?;
                        let end = row
                            .offset
                            .checked_add(5)
                            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
                        let displacement = checked_delta(target, end)?;
                        let encoded = isa_x86_64::encode_x86_64_selected_jump_form(
                            physical,
                            row.alternative,
                            displacement,
                        )
                        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                        if row.bytes.len() != 5
                            || encoded.footprint().encoded != jump.decoded_effects
                        {
                            return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                                row.instruction,
                            ));
                        }
                        jump.target_offset = target;
                        jump.byte_displacement = displacement;
                        row.bytes = encoded.bytes().to_vec();
                        continue;
                    }
                };
                rewrite_branch_offsets(
                    branch,
                    row.offset,
                    row.bytes.len(),
                    &offsets,
                    row.instruction,
                )?;
                let encoded = match (branch.predicate, row.bytes.len()) {
                    (ResolvedConditionalBranchPredicate::NonZeroV1, 2) => {
                        encode_x86_64_selected_short_nonzero_branch_form(
                            physical,
                            row.alternative,
                            branch.byte_displacement,
                        )
                    }
                    (ResolvedConditionalBranchPredicate::NonZeroV1, 6) => {
                        encode_x86_64_selected_nonzero_branch_form(
                            physical,
                            row.alternative,
                            branch.byte_displacement,
                        )
                    }
                    (ResolvedConditionalBranchPredicate::U64LessThanV1, 2 | 6) => {
                        encode_x86_64_selected_u64_less_than_branch_form(
                            physical,
                            row.alternative,
                            branch.byte_displacement,
                        )
                    }
                    (ResolvedConditionalBranchPredicate::I64LessThanV1, 2 | 6) => {
                        encode_x86_64_selected_i64_less_than_branch_form(
                            physical,
                            row.alternative,
                            branch.byte_displacement,
                        )
                    }
                    _ => {
                        return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                            row.instruction,
                        ));
                    }
                }
                .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                if encoded.bytes().len() != row.bytes.len() {
                    return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                        row.instruction,
                    ));
                }
                if encoded.footprint().encoded != branch.decoded_effects {
                    return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                        row.instruction,
                    ));
                }
                row.bytes = encoded.bytes().to_vec();
            }
        }
    }
    Ok(())
}

fn assign_dense_offsets(
    function: &mut ResolvedSelectedFunctionLayout,
) -> Result<BTreeMap<SelectedBlockId, u64>, OptimizedX86BranchRelaxationError> {
    let mut offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in &mut function.blocks {
        block.offset = offset;
        offsets.insert(block.block, offset);
        let start = offset;
        for row in &mut block.instructions {
            row.offset = offset;
            offset = offset
                .checked_add(
                    u64::try_from(row.bytes.len())
                        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                )
                .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        }
        block.byte_count = offset - start;
    }
    function.byte_count = offset;
    Ok(offsets)
}

fn rewrite_branch_offsets(
    branch: &mut machine_code::ResolvedConditionalBranchEvidence,
    instruction_offset: u64,
    instruction_size: usize,
    offsets: &BTreeMap<SelectedBlockId, u64>,
    instruction: SelectedInstructionId,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let taken = *offsets.get(&branch.when_taken_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = *offsets.get(&branch.when_fallthrough_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_fallthrough_block),
    )?;
    let end = instruction_offset
        .checked_add(
            u64::try_from(instruction_size)
                .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
        )
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    if fallthrough != end {
        return Err(OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(instruction));
    }
    branch.when_taken_offset = taken;
    branch.when_fallthrough_offset = fallthrough;
    branch.byte_displacement = checked_delta(taken, end)?;
    Ok(())
}
