use std::collections::BTreeMap;

use isa_aarch64::{
    encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    encode_aarch64_selected_i64_less_than_branch_form, encode_aarch64_selected_nonzero_branch_form,
    encode_aarch64_selected_u64_less_than_branch_form,
};
use isa_x86_64::{
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
};
use machine_code::{
    FunctionFragment, FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchEvidence,
    FunctionFragmentConditionalBranchPredicate,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedBlockId;
use target::Architecture;

use super::FrameApplicationError;

pub(super) fn reencode_branches(
    function: &mut FunctionFragment,
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), FrameApplicationError> {
    let block_offsets = function
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect::<BTreeMap<_, _>>();
    let function_bytes = &mut function.bytes;
    for block in &mut function.blocks {
        for row in &mut block.instructions {
            let instruction = row.instruction;
            let alternative = row.alternative;
            let row_offset = row.offset;
            let row_byte_count = row.bytes.len();
            let Some(branch) = row.branch.as_mut() else {
                continue;
            };
            let encoded = match branch.as_mut() {
                FunctionFragmentBranchEvidence::Jump(branch) => {
                    let target = block_offsets.get(&branch.target_block).copied().ok_or(
                        FrameApplicationError::MissingTargetBlock(branch.target_block),
                    )?;
                    let reference = match architecture {
                        Architecture::X86_64 => row_offset
                            .checked_add(row_byte_count as u64)
                            .ok_or(FrameApplicationError::OffsetOverflow)?,
                        Architecture::Aarch64 => row_offset,
                    };
                    branch.target_offset = target;
                    branch.byte_displacement = checked_delta(target, reference)?;
                    let (bytes, effects) = match architecture {
                        Architecture::X86_64 => {
                            let encoded = isa_x86_64::encode_x86_64_selected_jump_form(
                                physical,
                                alternative,
                                branch.byte_displacement,
                            )
                            .map_err(|error| {
                                FrameApplicationError::X86_64Branch(instruction, error)
                            })?;
                            (
                                encoded.bytes().to_vec(),
                                encoded.footprint().encoded.clone(),
                            )
                        }
                        Architecture::Aarch64 => {
                            let encoded = isa_aarch64::encode_aarch64_selected_jump_form(
                                physical,
                                alternative,
                                branch.byte_displacement,
                            )
                            .map_err(|error| {
                                FrameApplicationError::Aarch64Branch(instruction, error)
                            })?;
                            (
                                encoded.bytes().to_vec(),
                                encoded.footprint().encoded.clone(),
                            )
                        }
                    };
                    if bytes.len() != row_byte_count || effects != branch.decoded_effects {
                        return Err(FrameApplicationError::BranchEffectsMismatch(instruction));
                    }
                    bytes
                }
                FunctionFragmentBranchEvidence::Conditional(branch) => {
                    rewrite_branch_coordinates(
                        instruction,
                        row_offset,
                        row_byte_count,
                        branch,
                        architecture,
                        &block_offsets,
                    )?;
                    encode_branch(
                        instruction,
                        alternative,
                        row_byte_count,
                        branch,
                        architecture,
                        physical,
                    )?
                }
            };
            let start =
                usize::try_from(row_offset).map_err(|_| FrameApplicationError::OffsetOverflow)?;
            let end = start
                .checked_add(encoded.len())
                .ok_or(FrameApplicationError::OffsetOverflow)?;
            let destination = function_bytes
                .get_mut(start..end)
                .ok_or(FrameApplicationError::SourceShapeMismatch(function.machine))?;
            destination.copy_from_slice(&encoded);
            row.bytes = encoded;
        }
    }
    Ok(())
}

fn rewrite_branch_coordinates(
    instruction: selected_instructions::SelectedInstructionId,
    row_offset: u64,
    row_byte_count: usize,
    branch: &mut FunctionFragmentConditionalBranchEvidence,
    architecture: Architecture,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
) -> Result<(), FrameApplicationError> {
    let taken = block_offsets.get(&branch.when_taken_block).copied().ok_or(
        FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = block_offsets
        .get(&branch.when_fallthrough_block)
        .copied()
        .ok_or(FrameApplicationError::MissingTargetBlock(
            branch.when_fallthrough_block,
        ))?;
    let row_len =
        u64::try_from(row_byte_count).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let end = row_offset
        .checked_add(row_len)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    if fallthrough != end {
        return Err(FrameApplicationError::BranchFallthroughMismatch(
            instruction,
        ));
    }
    branch.when_taken_offset = taken;
    branch.when_fallthrough_offset = fallthrough;
    branch.byte_displacement = checked_delta(
        taken,
        match architecture {
            Architecture::X86_64 => end,
            Architecture::Aarch64 => row_offset,
        },
    )?;
    Ok(())
}

fn encode_branch(
    instruction: selected_instructions::SelectedInstructionId,
    alternative: selected_instructions::MachineAlternativeKey,
    row_byte_count: usize,
    branch: &FunctionFragmentConditionalBranchEvidence,
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<u8>, FrameApplicationError> {
    let (bytes, register_reads, effects) = match (architecture, branch.predicate) {
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::NonZeroV1) => {
            let encoded = match row_byte_count {
                2 => encode_x86_64_selected_short_nonzero_branch_form(
                    physical,
                    alternative,
                    branch.byte_displacement,
                ),
                _ => encode_x86_64_selected_nonzero_branch_form(
                    physical,
                    alternative,
                    branch.byte_displacement,
                ),
            }
            .map_err(|error| FrameApplicationError::X86_64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::U64LessThanV1) => {
            let encoded = encode_x86_64_selected_u64_less_than_branch_form(
                physical,
                alternative,
                branch.byte_displacement,
            )
            .map_err(|error| FrameApplicationError::X86_64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::X86_64, FunctionFragmentConditionalBranchPredicate::I64LessThanV1) => {
            let encoded = encode_x86_64_selected_i64_less_than_branch_form(
                physical,
                alternative,
                branch.byte_displacement,
            )
            .map_err(|error| FrameApplicationError::X86_64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::NonZeroV1)
            if branch.decoded_register_reads.len() == 1 =>
        {
            let encoded = encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                physical,
                branch.decoded_register_reads[0],
                branch.byte_displacement,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::NonZeroV1) => {
            let encoded = encode_aarch64_selected_nonzero_branch_form(
                physical,
                alternative,
                branch.byte_displacement,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::U64LessThanV1) => {
            let encoded = encode_aarch64_selected_u64_less_than_branch_form(
                physical,
                alternative,
                branch.byte_displacement,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
        (Architecture::Aarch64, FunctionFragmentConditionalBranchPredicate::I64LessThanV1) => {
            let encoded = encode_aarch64_selected_i64_less_than_branch_form(
                physical,
                alternative,
                branch.byte_displacement,
            )
            .map_err(|error| FrameApplicationError::Aarch64Branch(instruction, error))?;
            (
                encoded.bytes().to_vec(),
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }
    };
    if bytes.len() != row_byte_count
        || register_reads != branch.decoded_register_reads
        || effects != branch.decoded_effects
    {
        return Err(FrameApplicationError::BranchEffectsMismatch(instruction));
    }
    Ok(bytes)
}

fn checked_delta(target: u64, reference: u64) -> Result<i64, FrameApplicationError> {
    let target = i128::from(target);
    let reference = i128::from(reference);
    i64::try_from(target - reference).map_err(|_| FrameApplicationError::OffsetOverflow)
}
